use crate::dll_dependencies;
use crate::mods::{DependencyIdentifier, Meta, Ordering, ZtdType};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

/// Result of dependency resolution
#[derive(Debug, Clone)]
pub struct ResolutionResult {
    pub order: Vec<String>,
    pub warnings: Vec<ResolutionWarning>,
}

/// Warnings generated during dependency resolution
#[derive(Debug, Clone)]
pub enum ResolutionWarning {
    CircularDependency { cycle: Vec<String> },
    TrulyCyclicDependency { cycle: Vec<String> },
    FormerlyCyclicDependency { mod_id: String, reason: String },
    MissingOptionalDependency { mod_id: String, missing: String },
    MissingRequiredDependency { mod_id: String, missing: String },
    ConflictingConstraints { mod_id: String, details: String },
}

/// Controls which dependencies to include when building the graph
#[derive(Debug, Clone, Copy)]
enum DependencyInclusionMode {
    /// Include both required and optional dependencies
    All,
    /// Include only required dependencies
    RequiredOnly,
}

/// Dependency graph for mod load order resolution
#[derive(Debug)]
struct DependencyGraph {
    // mod_id → mods that must load before this one
    before_deps: HashMap<String, Vec<String>>,

    // mod_id → mods that must load after this one
    after_deps: HashMap<String, Vec<String>>,

    // Track which dependencies are optional
    optional: HashMap<String, HashSet<String>>,
}

/// Manages dependency resolution for mod loading
pub struct DependencyResolver {
    mods: HashMap<String, Meta>,
    // Mapping from ztd_name to mod_id for identifier resolution
    ztd_to_mod_id: HashMap<String, String>,
}

impl DependencyResolver {
    /// Create a new resolver with available mods
    ///
    /// # Arguments
    /// * `mods` - HashMap of mod_id to Meta
    /// * `discovered` - HashMap of mod_id to (ztd_name, Meta) for resolving ztd_name dependencies
    pub fn new(mods: HashMap<String, Meta>, discovered: &HashMap<String, (String, Meta)>) -> Self {
        let ztd_to_mod_id = discovered.iter().map(|(mod_id, (ztd_name, _))| (ztd_name.clone(), mod_id.clone())).collect();

        Self { mods, ztd_to_mod_id }
    }

    /// Resolve mod load order based on dependencies and existing configuration
    ///
    /// # Arguments
    /// * `existing_order` - Current mod order from openzt.toml (user-controlled)
    /// * `disabled_mods` - Mods that should not be loaded (but kept in order)
    /// * `pure_legacy_in_mods` - Pure legacy archives (no meta.toml) found in /mods/ directory
    ///
    /// # Returns
    /// Resolution result with final order and any warnings
    ///
    /// Note: Disabled mods are kept in the order list but not processed for dependencies.
    /// They will be filtered out during actual mod loading.
    pub fn resolve_order(&self, existing_order: &[String], disabled_mods: &[String], pure_legacy_in_mods: &[(String, PathBuf)]) -> ResolutionResult {
        let mut warnings = Vec::new();

        // Disabled mods should be kept in order but not processed
        let disabled_set: HashSet<_> = disabled_mods.iter().cloned().collect();

        // Filter out disabled pure legacy archives
        let enabled_pure_legacy: Vec<_> = pure_legacy_in_mods.iter().filter(|(filename, _)| !disabled_set.contains(filename)).cloned().collect();

        // Identify new pure legacy archives (not in existing order)
        let existing_set: HashSet<_> = existing_order.iter().cloned().collect();
        let new_pure_legacy: Vec<_> = enabled_pure_legacy.iter().filter(|(filename, _)| !existing_set.contains(filename)).cloned().collect();

        // Identify new OpenZT mods (not in existing order and not disabled)
        let mut new_mods: Vec<_> = self
            .mods
            .keys()
            .filter(|id| !existing_set.contains(*id) && !disabled_set.contains(*id))
            .cloned()
            .collect();

        // Sort new mods alphabetically for deterministic processing
        new_mods.sort();

        // Keep all entries in existing_order, but log errors for missing ones
        // Only check existence to identify new archives for insertion
        for entry in existing_order.iter() {
            // Check if it's a valid OpenZT mod
            let is_openzt_mod = self.mods.contains_key(entry);
            // Check if it's a valid pure legacy archive (by filename)
            let is_pure_legacy = entry.to_lowercase().ends_with(".ztd") && pure_legacy_in_mods.iter().any(|(filename, _)| filename == entry);

            if !is_openzt_mod && !is_pure_legacy {
                // Entry doesn't exist - log error but keep it in order
                error!("Mod '{}' not found: could not find mod in /mods/ directory", entry);
            }
        }

        // Categorize new OpenZT mods
        let mut legacy_type_no_deps: Vec<String> = Vec::new();
        let mut mods_with_deps: Vec<String> = Vec::new();

        for mod_id in &new_mods {
            if let Some(meta) = self.mods.get(mod_id) {
                if *meta.ztd_type() == ZtdType::Legacy && meta.dependencies().is_empty() {
                    legacy_type_no_deps.push(mod_id.clone());
                } else {
                    mods_with_deps.push(mod_id.clone());
                }
            }
        }

        debug!("Discovered {} new pure legacy archive(s) in /mods/", new_pure_legacy.len());
        debug!("Discovered {} new ztd_type='legacy' mod(s) with no deps", legacy_type_no_deps.len());
        debug!("Discovered {} new mod(s) with deps or other ztd_type", mods_with_deps.len());

        // Build dependency graph only for enabled mods with dependencies
        let enabled_mods_with_deps: HashMap<_, _> = self
            .mods
            .iter()
            .filter(|(id, _)| !disabled_set.contains(*id) && mods_with_deps.contains(*id))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let graph = if !enabled_mods_with_deps.is_empty() {
            self.build_dependency_graph(&enabled_mods_with_deps, DependencyInclusionMode::All)
        } else {
            DependencyGraph {
                before_deps: HashMap::new(),
                after_deps: HashMap::new(),
                optional: HashMap::new(),
            }
        };

        // Two-stage cycle detection for mods with deps
        let (truly_cyclic, formerly_cyclic, stage1_cycles, stage2_cycles) = if !mods_with_deps.is_empty() {
            self.detect_cycles_two_stage(&graph, &mods_with_deps, &enabled_mods_with_deps)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

        // Generate warnings for Stage 1 cycles (warning level)
        for cycle in &stage1_cycles {
            warn!("Circular dependency detected (with optional deps): {:?}", cycle);
            warnings.push(ResolutionWarning::CircularDependency { cycle: cycle.clone() });
        }

        // Generate errors for Stage 2 cycles (error level)
        for cycle in &stage2_cycles {
            error!("Truly cyclic dependency (required deps only): {:?}", cycle);
            warnings.push(ResolutionWarning::TrulyCyclicDependency { cycle: cycle.clone() });
        }

        // Generate warnings for formerly cyclic mods (info/warning level)
        for mod_id in &formerly_cyclic {
            info!("Mod '{}' cycle resolved by ignoring optional dependencies", mod_id);
            warnings.push(ResolutionWarning::FormerlyCyclicDependency {
                mod_id: mod_id.clone(),
                reason: "Cycle resolved by ignoring optional dependencies".to_string(),
            });
        }

        // Insert new entries into existing order
        // Priority: pure legacy -> legacy-no-deps -> resolved mods
        let (final_order, insert_warnings) = self.insert_new_entries(
            existing_order,
            &new_pure_legacy,
            &legacy_type_no_deps,
            &mods_with_deps,
            &graph,
            &enabled_mods_with_deps,
            &truly_cyclic,
            &formerly_cyclic,
        );

        warnings.extend(insert_warnings);

        ResolutionResult { order: final_order, warnings }
    }

    /// Build dependency graph from mod metadata
    fn build_dependency_graph(&self, mods: &HashMap<String, Meta>, mode: DependencyInclusionMode) -> DependencyGraph {
        let mut before_deps: HashMap<String, Vec<String>> = HashMap::new();
        let mut after_deps: HashMap<String, Vec<String>> = HashMap::new();
        let mut optional: HashMap<String, HashSet<String>> = HashMap::new();

        for (mod_id, meta) in mods {
            for dep in meta.dependencies() {
                // Warn if min_version is specified with non-mod_id identifiers
                if dep.min_version().is_some() {
                    match dep.identifier() {
                        DependencyIdentifier::ModId(_) => {
                            // min_version is valid for mod_id
                        }
                        DependencyIdentifier::ZtdName(name) => {
                            warn!(
                                "Dependency '{}' (ztd_name) has min_version specified, which is not supported for ztd_name dependencies. Version will be ignored.",
                                name
                            );
                        }
                        DependencyIdentifier::DllName(name) => {
                            warn!(
                                "Dependency '{}' (dll_name) has min_version specified, which is not supported for dll_name dependencies. Version will be ignored.",
                                name
                            );
                        }
                    }
                }

                // Resolve the identifier to a mod_id (or handle DLL dependencies)
                let resolved_id = match dep.identifier() {
                    DependencyIdentifier::ModId(id) => id.clone(),
                    DependencyIdentifier::ZtdName(ztd_name) => {
                        match self.ztd_to_mod_id.get(ztd_name) {
                            Some(id) => id.clone(),
                            None => {
                                // ztd_name not found - use the ztd_name as identifier
                                // This will be handled in find_insert_position to generate appropriate warnings
                                debug!("ztd_name dependency '{}' for mod '{}' not found", ztd_name, mod_id);
                                ztd_name.clone()
                            }
                        }
                    }
                    DependencyIdentifier::DllName(dll_name) => {
                        // Validate DLL dependency
                        let dll_exists = dll_dependencies::check_dll_dependency(dll_name);
                        if !dll_exists {
                            if *dep.optional() {
                                debug!("Optional DLL dependency '{}' not found", dll_name);
                            } else {
                                warn!("Required DLL dependency '{}' not found", dll_name);
                                // Add to warnings but don't affect load ordering
                            }
                        }
                        // DLL dependencies don't participate in load ordering
                        // Track optional status for consistency
                        if *dep.optional() {
                            optional.entry(mod_id.clone()).or_default().insert(dll_name.clone());
                        }
                        continue;
                    }
                };

                // Check if we should include this dependency based on the mode
                let should_include = match mode {
                    DependencyInclusionMode::All => true,
                    DependencyInclusionMode::RequiredOnly => !dep.optional(),
                };

                if !should_include {
                    // Track optional deps even if we're not including their edges
                    if *dep.optional() {
                        optional.entry(mod_id.clone()).or_default().insert(resolved_id.clone());
                    }
                    continue;
                }

                match dep.ordering() {
                    Ordering::Before => {
                        // This mod must load BEFORE resolved_id
                        after_deps.entry(mod_id.clone()).or_default().push(resolved_id.clone());
                        before_deps.entry(resolved_id.clone()).or_default().push(mod_id.clone());
                    }
                    Ordering::After => {
                        // This mod must load AFTER resolved_id
                        before_deps.entry(mod_id.clone()).or_default().push(resolved_id.clone());
                        after_deps.entry(resolved_id.clone()).or_default().push(mod_id.clone());
                    }
                    Ordering::None => {
                        // No ordering constraint, just track dependency existence
                    }
                }

                if *dep.optional() {
                    optional.entry(mod_id.clone()).or_default().insert(resolved_id.clone());
                }
            }
        }

        DependencyGraph {
            before_deps,
            after_deps,
            optional,
        }
    }

    /// Detect cycles using Tarjan's strongly connected components algorithm
    /// Only considers the specified subset of mods
    fn detect_cycles_in_subgraph(&self, graph: &DependencyGraph, mods_to_check: &[String]) -> Vec<Vec<String>> {
        let mod_set: HashSet<_> = mods_to_check.iter().cloned().collect();

        let mut state = TarjanState::new();
        let mut sccs = Vec::new();

        for mod_id in mods_to_check {
            if !state.indices.contains_key(mod_id) {
                self.tarjan_strongconnect(mod_id, graph, &mod_set, &mut state, &mut sccs);
            }
        }

        // Filter to only cycles (SCC size > 1)
        sccs.into_iter().filter(|component| component.len() > 1).collect()
    }

    /// Two-stage cycle detection:
    /// 1. Detect cycles with all dependencies (required + optional)
    /// 2. For cyclic mods, re-check with only required dependencies
    ///
    /// Returns: (truly_cyclic_mods, formerly_cyclic_mods, stage1_cycles, stage2_cycles)
    #[allow(clippy::type_complexity)]
    fn detect_cycles_two_stage(
        &self,
        all_deps_graph: &DependencyGraph,
        mods_to_check: &[String],
        all_mods: &HashMap<String, Meta>,
    ) -> (Vec<String>, Vec<String>, Vec<Vec<String>>, Vec<Vec<String>>) {
        // Stage 1: Detect cycles with ALL dependencies
        let stage1_cycles = self.detect_cycles_in_subgraph(all_deps_graph, mods_to_check);

        if stage1_cycles.is_empty() {
            // No cycles at all
            return (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        }

        // Collect all mods involved in Stage 1 cycles
        let stage1_cyclic_mods: HashSet<String> = stage1_cycles.iter().flat_map(|cycle| cycle.iter().cloned()).collect();

        info!(
            "Stage 1: Detected {} cycle(s) involving {} mods (with optional deps)",
            stage1_cycles.len(),
            stage1_cyclic_mods.len()
        );

        for cycle in &stage1_cycles {
            debug!("  Stage 1 cycle: {:?}", cycle);
        }

        // Stage 2: Build required-only graph for cyclic mods
        let cyclic_mods_only: HashMap<String, Meta> = all_mods
            .iter()
            .filter(|(id, _)| stage1_cyclic_mods.contains(*id))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        info!("Stage 2: Re-checking {} mods with required-only dependencies...", stage1_cyclic_mods.len());

        let required_only_graph = self.build_dependency_graph(&cyclic_mods_only, DependencyInclusionMode::RequiredOnly);

        let cyclic_mod_ids: Vec<_> = stage1_cyclic_mods.iter().cloned().collect();
        let stage2_cycles = self.detect_cycles_in_subgraph(&required_only_graph, &cyclic_mod_ids);

        info!(
            "Stage 2: Detected {} cycle(s) with {} mods still cyclic",
            stage2_cycles.len(),
            stage2_cycles.iter().flat_map(|c| c.iter()).count()
        );

        for cycle in &stage2_cycles {
            debug!("  Stage 2 cycle: {:?}", cycle);
        }

        // Categorize: mods in Stage 2 cycles are truly cyclic, others are formerly cyclic
        let stage2_cyclic_mods: HashSet<String> = stage2_cycles.iter().flat_map(|cycle| cycle.iter().cloned()).collect();

        let truly_cyclic: Vec<_> = stage2_cyclic_mods.iter().cloned().collect();
        let formerly_cyclic: Vec<_> = stage1_cyclic_mods.difference(&stage2_cyclic_mods).cloned().collect();

        info!("Result: {} truly cyclic, {} formerly cyclic (resolved)", truly_cyclic.len(), formerly_cyclic.len());

        (truly_cyclic, formerly_cyclic, stage1_cycles, stage2_cycles)
    }

    /// Topologically sort a list of mods using Kahn's algorithm
    /// Returns mods in dependency order (mods with no deps first)
    fn topological_sort(&self, mods: &[String], graph: &DependencyGraph) -> Vec<String> {
        let mod_set: HashSet<_> = mods.iter().cloned().collect();

        // Calculate in-degree for each mod (number of dependencies)
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for mod_id in mods {
            in_degree.insert(mod_id.clone(), 0);
        }

        // Count in-degrees based on before_deps (things this mod depends on)
        for mod_id in mods {
            if let Some(deps) = graph.before_deps.get(mod_id) {
                for dep in deps {
                    if mod_set.contains(dep) {
                        *in_degree.entry(mod_id.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        // Start with mods that have no dependencies
        let mut queue: Vec<_> = in_degree.iter().filter(|(_, &degree)| degree == 0).map(|(id, _)| id.clone()).collect();
        queue.sort(); // Alphabetical for determinism

        let mut result = Vec::new();

        while !queue.is_empty() {
            // Remove from front to maintain order (FIFO)
            let mod_id = queue.remove(0);
            result.push(mod_id.clone());

            // Reduce in-degree for mods that depend on this one
            if let Some(dependents) = graph.after_deps.get(&mod_id) {
                for dependent in dependents {
                    if !mod_set.contains(dependent) {
                        continue;
                    }

                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(dependent.clone());
                            queue.sort(); // Keep alphabetical
                        }
                    }
                }
            }
        }

        // If we didn't process all mods, there's still a cycle (shouldn't happen for formerly cyclic)
        // Fall back to alphabetical order
        if result.len() != mods.len() {
            debug!("Topological sort incomplete, falling back to alphabetical order");
            let mut fallback = mods.to_vec();
            fallback.sort();
            return fallback;
        }

        result
    }

    /// Tarjan's algorithm recursive step
    #[allow(clippy::only_used_in_recursion)]
    fn tarjan_strongconnect(&self, mod_id: &str, graph: &DependencyGraph, mod_set: &HashSet<String>, state: &mut TarjanState, sccs: &mut Vec<Vec<String>>) {
        let index = state.index;
        state.indices.insert(mod_id.to_string(), index);
        state.lowlinks.insert(mod_id.to_string(), index);
        state.index += 1;
        state.stack.push(mod_id.to_string());
        state.on_stack.insert(mod_id.to_string());

        // Consider dependencies (nodes this mod depends on)
        if let Some(deps) = graph.before_deps.get(mod_id) {
            for dep in deps {
                // Only follow edges within the subgraph
                if !mod_set.contains(dep) {
                    continue;
                }

                if !state.indices.contains_key(dep) {
                    self.tarjan_strongconnect(dep, graph, mod_set, state, sccs);
                    let dep_lowlink = state.lowlinks[dep];
                    let current_lowlink = state.lowlinks.get_mut(mod_id).unwrap();
                    *current_lowlink = (*current_lowlink).min(dep_lowlink);
                } else if state.on_stack.contains(dep) {
                    let dep_index = state.indices[dep];
                    let current_lowlink = state.lowlinks.get_mut(mod_id).unwrap();
                    *current_lowlink = (*current_lowlink).min(dep_index);
                }
            }
        }

        // If mod_id is a root node, pop the stack and create an SCC
        if state.lowlinks[mod_id] == state.indices[mod_id] {
            let mut component = Vec::new();
            loop {
                let node = state.stack.pop().unwrap();
                state.on_stack.remove(&node);
                component.push(node.clone());
                if node == mod_id {
                    break;
                }
            }
            sccs.push(component);
        }
    }

    /// Insert new mods into existing order respecting dependencies
    fn insert_new_mods(
        &self,
        existing_order: &[String],
        new_mods: &[String],
        graph: &DependencyGraph,
        all_mods: &HashMap<String, Meta>,
        truly_cyclic: &[String],
        formerly_cyclic: &[String],
    ) -> (Vec<String>, Vec<ResolutionWarning>) {
        let mut order = existing_order.to_vec();
        let mut warnings = Vec::new();

        // Build sets for efficient lookup
        let truly_cyclic_set: HashSet<_> = truly_cyclic.iter().cloned().collect();
        let formerly_cyclic_set: HashSet<_> = formerly_cyclic.iter().cloned().collect();

        // Categorize new mods into three groups
        let mut never_cyclic: Vec<_> = new_mods
            .iter()
            .filter(|id| !truly_cyclic_set.contains(*id) && !formerly_cyclic_set.contains(*id))
            .cloned()
            .collect();

        let mut formerly_cyclic_sorted = formerly_cyclic.to_vec();
        let mut truly_cyclic_sorted = truly_cyclic.to_vec();

        // Sort all for determinism
        never_cyclic.sort();
        formerly_cyclic_sorted.sort();
        truly_cyclic_sorted.sort();

        // Build required-only graph for formerly cyclic mods
        let formerly_cyclic_mods: HashMap<_, _> = all_mods
            .iter()
            .filter(|(id, _)| formerly_cyclic_set.contains(*id))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let required_only_graph = self.build_dependency_graph(&formerly_cyclic_mods, DependencyInclusionMode::RequiredOnly);

        // Insert never-cyclic mods first (using full graph)
        // Track the number of mods inserted at the beginning to maintain alphabetical order
        let mut insert_offset = 0;

        for mod_id in never_cyclic {
            let (position, insert_warnings) = self.find_insert_position(&mod_id, &order, graph, all_mods);
            warnings.extend(insert_warnings);

            // If inserting at the beginning with no constraints, use insert_offset
            // to maintain alphabetical order among independent mods
            let actual_position = if position == 0 && insert_offset > 0 { position + insert_offset } else { position };

            order.insert(actual_position, mod_id);

            // Track insertions at position 0 for alphabetical ordering
            if position == 0 {
                insert_offset += 1;
            }
        }

        // Topologically sort formerly cyclic mods using required-only dependencies
        let formerly_cyclic_order = self.topological_sort(&formerly_cyclic_sorted, &required_only_graph);

        // Insert all formerly cyclic mods as a group in their sorted order
        // They go after never-cyclic mods but before truly cyclic mods
        for mod_id in formerly_cyclic_order {
            info!("Inserting formerly cyclic mod '{}' (acyclic without optional deps)", mod_id);
            order.push(mod_id);
        }

        // Append truly cyclic mods at end (already sorted alphabetically)
        for mod_id in truly_cyclic_sorted {
            info!("Inserting truly cyclic mod '{}' at end (cyclic even without optional deps)", mod_id);
            order.push(mod_id);
        }

        (order, warnings)
    }

    /// Find the best insertion position for a new mod
    fn find_insert_position(
        &self,
        mod_id: &str,
        current_order: &[String],
        graph: &DependencyGraph,
        all_mods: &HashMap<String, Meta>,
    ) -> (usize, Vec<ResolutionWarning>) {
        let mut warnings = Vec::new();
        let mut min_position = 0;
        let mut max_position = current_order.len();

        // Create position map for efficient lookup
        let position_map: HashMap<&String, usize> = current_order.iter().enumerate().map(|(i, id)| (id, i)).collect();

        // Check "before" dependencies (mods this one depends on)
        if let Some(before_deps) = graph.before_deps.get(mod_id) {
            for dep in before_deps {
                if let Some(&pos) = position_map.get(dep) {
                    // This mod must load AFTER dep, so min_position is after dep
                    min_position = min_position.max(pos + 1);
                } else if !all_mods.contains_key(dep) {
                    // Dependency doesn't exist
                    let meta = &all_mods[mod_id];
                    // Check if this unresolved dependency is optional
                    let is_optional = meta.dependencies().iter().any(|d| match d.identifier() {
                        DependencyIdentifier::ModId(id) => id == dep && *d.optional(),
                        DependencyIdentifier::ZtdName(name) => name == dep && *d.optional(),
                        DependencyIdentifier::DllName(name) => name == dep && *d.optional(),
                    });

                    if is_optional {
                        debug!("Optional dependency '{}' for mod '{}' not found", dep, mod_id);
                        warnings.push(ResolutionWarning::MissingOptionalDependency {
                            mod_id: mod_id.to_string(),
                            missing: dep.to_string(),
                        });
                    } else {
                        warn!("Required dependency '{}' for mod '{}' not found", dep, mod_id);
                        warnings.push(ResolutionWarning::MissingRequiredDependency {
                            mod_id: mod_id.to_string(),
                            missing: dep.to_string(),
                        });
                    }
                }
            }
        }

        // Check "after" dependencies (mods that depend on this one)
        if let Some(after_deps) = graph.after_deps.get(mod_id) {
            for dep in after_deps {
                if let Some(&pos) = position_map.get(dep) {
                    // This mod must load BEFORE dep, so max_position is before dep
                    max_position = max_position.min(pos);
                }
            }
        }

        // Determine final position
        let position = if min_position > max_position {
            // Conflicting constraints
            warn!(
                "Conflicting dependency constraints for mod '{}': must be in range [{}, {}]",
                mod_id, min_position, max_position
            );
            warnings.push(ResolutionWarning::ConflictingConstraints {
                mod_id: mod_id.to_string(),
                details: format!("Required position range [{}, {}] is invalid", min_position, max_position),
            });
            // Insert at end as fallback
            current_order.len()
        } else {
            // Insert at earliest valid position
            min_position
        };

        debug!("Inserting mod '{}' at position {}", mod_id, position);
        (position, warnings)
    }

    /// Insert new entries (pure legacy, legacy-no-deps, and mods with deps) into existing order
    ///
    /// Priority:
    /// 1. Pure legacy archives (no meta.toml) - inserted at position 0 alphabetically
    /// 2. ztd_type="legacy" with no deps - inserted at position 0 alphabetically
    /// 3. Mods with deps or other ztd_type - resolved via dependency graph
    fn insert_new_entries(
        &self,
        existing_order: &[String],
        new_pure_legacy: &[(String, PathBuf)],
        legacy_type_no_deps: &[String],
        mods_with_deps: &[String],
        graph: &DependencyGraph,
        all_mods: &HashMap<String, Meta>,
        truly_cyclic: &[String],
        formerly_cyclic: &[String],
    ) -> (Vec<String>, Vec<ResolutionWarning>) {
        let mut order = existing_order.to_vec();
        let mut warnings = Vec::new();

        // Build sets for efficient lookup
        let truly_cyclic_set: HashSet<_> = truly_cyclic.iter().cloned().collect();
        let formerly_cyclic_set: HashSet<_> = formerly_cyclic.iter().cloned().collect();

        // Step 1: Insert pure legacy archives at position 0 (alphabetically sorted)
        // Note: We reverse before inserting because inserting at position 0 in a loop
        // would otherwise reverse the order
        let mut pure_legacy_filenames: Vec<String> = new_pure_legacy.iter().map(|(filename, _)| filename.clone()).collect();
        pure_legacy_filenames.sort_by_key(|a| a.to_lowercase());
        pure_legacy_filenames.reverse(); // Reverse so they end up in correct order after insert(0)

        for filename in pure_legacy_filenames {
            order.insert(0, filename);
            debug!("Inserted pure legacy archive '{}' at position 0", order[0]);
        }

        // Step 2: Insert ztd_type="legacy" with no deps at position 0 (alphabetically)
        // These go after pure legacy archives (so before them in the final order)
        let mut legacy_no_deps_sorted = legacy_type_no_deps.to_vec();
        legacy_no_deps_sorted.sort();
        legacy_no_deps_sorted.reverse(); // Reverse so they end up in correct order after insert(0)

        for mod_id in legacy_no_deps_sorted {
            order.insert(0, mod_id);
            info!("Inserted ztd_type='legacy' mod (no deps) '{}' at position 0", order[0]);
        }

        // Step 3: Categorize mods with deps
        let mut never_cyclic: Vec<_> = mods_with_deps
            .iter()
            .filter(|id| !truly_cyclic_set.contains(*id) && !formerly_cyclic_set.contains(*id))
            .cloned()
            .collect();

        let mut formerly_cyclic_sorted = formerly_cyclic.to_vec();
        let mut truly_cyclic_sorted = truly_cyclic.to_vec();

        // Sort all for determinism
        never_cyclic.sort();
        formerly_cyclic_sorted.sort();
        truly_cyclic_sorted.sort();

        // Build required-only graph for formerly cyclic mods
        let formerly_cyclic_mods: HashMap<_, _> = all_mods
            .iter()
            .filter(|(id, _)| formerly_cyclic_set.contains(*id))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let required_only_graph = if !formerly_cyclic_mods.is_empty() {
            self.build_dependency_graph(&formerly_cyclic_mods, DependencyInclusionMode::RequiredOnly)
        } else {
            DependencyGraph {
                before_deps: HashMap::new(),
                after_deps: HashMap::new(),
                optional: HashMap::new(),
            }
        };

        // Insert never-cyclic mods (using full graph)
        let mut insert_offset = 0;

        for mod_id in never_cyclic {
            let (position, insert_warnings) = self.find_insert_position(&mod_id, &order, graph, all_mods);
            warnings.extend(insert_warnings);

            // If inserting at the beginning with no constraints, use insert_offset
            // to maintain alphabetical order among independent mods
            let actual_position = if position == 0 && insert_offset > 0 { position + insert_offset } else { position };

            order.insert(actual_position, mod_id);

            // Track insertions at position 0 for alphabetical ordering
            if position == 0 {
                insert_offset += 1;
            }
        }

        // Topologically sort formerly cyclic mods using required-only dependencies
        let formerly_cyclic_order = self.topological_sort(&formerly_cyclic_sorted, &required_only_graph);

        // Insert all formerly cyclic mods as a group in their sorted order
        // They go after never-cyclic mods but before truly cyclic mods
        for mod_id in formerly_cyclic_order {
            info!("Inserting formerly cyclic mod '{}' (acyclic without optional deps)", mod_id);
            order.push(mod_id);
        }

        // Append truly cyclic mods at end (already sorted alphabetically)
        for mod_id in truly_cyclic_sorted {
            info!("Inserting truly cyclic mod '{}' at end (cyclic even without optional deps)", mod_id);
            order.push(mod_id);
        }

        (order, warnings)
    }
}

/// State for Tarjan's algorithm
struct TarjanState {
    index: usize,
    stack: Vec<String>,
    indices: HashMap<String, usize>,
    lowlinks: HashMap<String, usize>,
    on_stack: HashSet<String>,
}

impl TarjanState {
    fn new() -> Self {
        Self {
            index: 0,
            stack: Vec::new(),
            indices: HashMap::new(),
            lowlinks: HashMap::new(),
            on_stack: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::Meta;

    /// Helper to create test metadata from TOML string
    fn create_test_meta(toml_str: &str) -> Meta {
        toml::from_str(toml_str).expect("Failed to parse test TOML")
    }

    #[test]
    fn test_empty_resolver() {
        let discovered = HashMap::new();
        let resolver = DependencyResolver::new(HashMap::new(), &discovered);
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&[], &[], pure_legacy);
        assert!(result.order.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_no_new_mods() {
        let mut mods = HashMap::new();

        // Create two mods
        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
        "#,
        );

        let meta_b = create_test_meta(
            r#"
            name = "Mod B"
            description = "Test mod B"
            authors = ["Test"]
            mod_id = "test.mod_b"
            version = "1.0.0"
        "#,
        );

        mods.insert("test.mod_a".to_string(), meta_a.clone());
        mods.insert("test.mod_b".to_string(), meta_b.clone());

        // Create discovered map matching the mods
        let mut discovered = HashMap::new();
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));
        discovered.insert("test.mod_b".to_string(), ("test.mod_b.ztd".to_string(), meta_b));

        let resolver = DependencyResolver::new(mods, &discovered);
        let existing = vec!["test.mod_a".to_string(), "test.mod_b".to_string()];
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&existing, &[], pure_legacy);

        // Should return existing order unchanged
        assert_eq!(result.order, existing);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_simple_after_dependency() {
        let mut mods = HashMap::new();

        // Mod A has no dependencies
        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
        "#,
        );

        // Mod B depends on A (must load AFTER A)
        let meta_b = create_test_meta(
            r#"
            name = "Mod B"
            description = "Test mod B"
            authors = ["Test"]
            mod_id = "test.mod_b"
            version = "1.0.0"
            dependencies = [
                { mod_id = "test.mod_a", name = "Mod A", ordering = "after" }
            ]
        "#,
        );

        mods.insert("test.mod_a".to_string(), meta_a.clone());
        mods.insert("test.mod_b".to_string(), meta_b.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));
        discovered.insert("test.mod_b".to_string(), ("test.mod_b.ztd".to_string(), meta_b));

        let resolver = DependencyResolver::new(mods, &discovered);
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&[], &[], pure_legacy);

        // mod_a should load before mod_b
        assert_eq!(result.order, vec!["test.mod_a", "test.mod_b"]);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_simple_before_dependency() {
        let mut mods = HashMap::new();

        // Mod A must load BEFORE B
        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
            dependencies = [
                { mod_id = "test.mod_b", name = "Mod B", ordering = "before" }
            ]
        "#,
        );

        // Mod B has no dependencies
        let meta_b = create_test_meta(
            r#"
            name = "Mod B"
            description = "Test mod B"
            authors = ["Test"]
            mod_id = "test.mod_b"
            version = "1.0.0"
        "#,
        );

        mods.insert("test.mod_a".to_string(), meta_a.clone());
        mods.insert("test.mod_b".to_string(), meta_b.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));
        discovered.insert("test.mod_b".to_string(), ("test.mod_b.ztd".to_string(), meta_b));

        let resolver = DependencyResolver::new(mods, &discovered);
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&[], &[], pure_legacy);

        // mod_a should load before mod_b
        assert_eq!(result.order, vec!["test.mod_a", "test.mod_b"]);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut mods = HashMap::new();

        // A depends on B (after)
        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
            dependencies = [
                { mod_id = "test.mod_b", name = "Mod B", ordering = "after" }
            ]
        "#,
        );

        // B depends on C (after)
        let meta_b = create_test_meta(
            r#"
            name = "Mod B"
            description = "Test mod B"
            authors = ["Test"]
            mod_id = "test.mod_b"
            version = "1.0.0"
            dependencies = [
                { mod_id = "test.mod_c", name = "Mod C", ordering = "after" }
            ]
        "#,
        );

        // C depends on A (after) - creates cycle!
        let meta_c = create_test_meta(
            r#"
            name = "Mod C"
            description = "Test mod C"
            authors = ["Test"]
            mod_id = "test.mod_c"
            version = "1.0.0"
            dependencies = [
                { mod_id = "test.mod_a", name = "Mod A", ordering = "after" }
            ]
        "#,
        );

        mods.insert("test.mod_a".to_string(), meta_a.clone());
        mods.insert("test.mod_b".to_string(), meta_b.clone());
        mods.insert("test.mod_c".to_string(), meta_c.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));
        discovered.insert("test.mod_b".to_string(), ("test.mod_b.ztd".to_string(), meta_b));
        discovered.insert("test.mod_c".to_string(), ("test.mod_c.ztd".to_string(), meta_c));

        let resolver = DependencyResolver::new(mods, &discovered);
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&[], &[], pure_legacy);

        // Should detect cycle and place mods at end in alphabetical order
        assert_eq!(result.order.len(), 3);
        assert!(result.order.contains(&"test.mod_a".to_string()));
        assert!(result.order.contains(&"test.mod_b".to_string()));
        assert!(result.order.contains(&"test.mod_c".to_string()));

        // Should have warnings from both stages (all required deps, so truly cyclic)
        // Stage 1: CircularDependency (cycle with optional deps)
        // Stage 2: TrulyCyclicDependency (cycle still exists with required-only)
        assert_eq!(result.warnings.len(), 2);

        let has_stage1_warning = result
            .warnings
            .iter()
            .any(|w| matches!(w, ResolutionWarning::CircularDependency { cycle } if cycle.len() == 3));
        let has_stage2_warning = result
            .warnings
            .iter()
            .any(|w| matches!(w, ResolutionWarning::TrulyCyclicDependency { cycle } if cycle.len() == 3));

        assert!(has_stage1_warning, "Expected CircularDependency warning from Stage 1");
        assert!(has_stage2_warning, "Expected TrulyCyclicDependency warning from Stage 2");
    }

    #[test]
    fn test_missing_optional_dependency() {
        let mut mods = HashMap::new();

        // Mod A has optional dependency on non-existent mod
        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
            dependencies = [
                { mod_id = "test.nonexistent", name = "Nonexistent", optional = true, ordering = "after" }
            ]
        "#,
        );

        mods.insert("test.mod_a".to_string(), meta_a.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));

        let resolver = DependencyResolver::new(mods, &discovered);
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&[], &[], pure_legacy);

        assert_eq!(result.order, vec!["test.mod_a"]);
        assert_eq!(result.warnings.len(), 1);
        match &result.warnings[0] {
            ResolutionWarning::MissingOptionalDependency { mod_id, missing } => {
                assert_eq!(mod_id, "test.mod_a");
                assert_eq!(missing, "test.nonexistent");
            }
            _ => panic!("Expected MissingOptionalDependency warning"),
        }
    }

    #[test]
    fn test_missing_required_dependency() {
        let mut mods = HashMap::new();

        // Mod A has required dependency on non-existent mod
        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
            dependencies = [
                { mod_id = "test.nonexistent", name = "Nonexistent", ordering = "after" }
            ]
        "#,
        );

        mods.insert("test.mod_a".to_string(), meta_a.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));

        let resolver = DependencyResolver::new(mods, &discovered);
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&[], &[], pure_legacy);

        assert_eq!(result.order, vec!["test.mod_a"]);
        assert_eq!(result.warnings.len(), 1);
        match &result.warnings[0] {
            ResolutionWarning::MissingRequiredDependency { mod_id, missing } => {
                assert_eq!(mod_id, "test.mod_a");
                assert_eq!(missing, "test.nonexistent");
            }
            _ => panic!("Expected MissingRequiredDependency warning"),
        }
    }

    #[test]
    fn test_insert_new_mod_into_existing_order() {
        let mut mods = HashMap::new();

        // Existing mods
        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
        "#,
        );

        let meta_c = create_test_meta(
            r#"
            name = "Mod C"
            description = "Test mod C"
            authors = ["Test"]
            mod_id = "test.mod_c"
            version = "1.0.0"
        "#,
        );

        // New mod B depends on A and comes before C
        let meta_b = create_test_meta(
            r#"
            name = "Mod B"
            description = "Test mod B"
            authors = ["Test"]
            mod_id = "test.mod_b"
            version = "1.0.0"
            dependencies = [
                { mod_id = "test.mod_a", name = "Mod A", ordering = "after" },
                { mod_id = "test.mod_c", name = "Mod C", ordering = "before" }
            ]
        "#,
        );

        mods.insert("test.mod_a".to_string(), meta_a.clone());
        mods.insert("test.mod_b".to_string(), meta_b.clone());
        mods.insert("test.mod_c".to_string(), meta_c.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));
        discovered.insert("test.mod_b".to_string(), ("test.mod_b.ztd".to_string(), meta_b));
        discovered.insert("test.mod_c".to_string(), ("test.mod_c.ztd".to_string(), meta_c));

        let resolver = DependencyResolver::new(mods, &discovered);
        let existing = vec!["test.mod_a".to_string(), "test.mod_c".to_string()];
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&existing, &[], pure_legacy);

        // B should be inserted between A and C
        assert_eq!(result.order, vec!["test.mod_a", "test.mod_b", "test.mod_c"]);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_disabled_mods_stay_in_order() {
        let mut mods = HashMap::new();

        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
        "#,
        );

        let meta_b = create_test_meta(
            r#"
            name = "Mod B"
            description = "Test mod B"
            authors = ["Test"]
            mod_id = "test.mod_b"
            version = "1.0.0"
        "#,
        );

        let meta_c = create_test_meta(
            r#"
            name = "Mod C"
            description = "Test mod C"
            authors = ["Test"]
            mod_id = "test.mod_c"
            version = "1.0.0"
        "#,
        );

        mods.insert("test.mod_a".to_string(), meta_a.clone());
        mods.insert("test.mod_b".to_string(), meta_b.clone());
        mods.insert("test.mod_c".to_string(), meta_c.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));
        discovered.insert("test.mod_b".to_string(), ("test.mod_b.ztd".to_string(), meta_b));
        discovered.insert("test.mod_c".to_string(), ("test.mod_c.ztd".to_string(), meta_c));

        let resolver = DependencyResolver::new(mods, &discovered);

        // B is in existing order but disabled - should stay in order
        // C is new and disabled - should NOT be added
        let existing = vec!["test.mod_a".to_string(), "test.mod_b".to_string()];
        let disabled = vec!["test.mod_b".to_string(), "test.mod_c".to_string()];
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&existing, &disabled, pure_legacy);

        // Both A and B should be in order (B is disabled but stays in order)
        // C is new and disabled, so not added
        assert_eq!(result.order, vec!["test.mod_a", "test.mod_b"]);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_alphabetical_ordering_for_independent_mods() {
        let mut mods = HashMap::new();

        // Three independent mods with no dependencies
        let meta_c = create_test_meta(
            r#"
            name = "Mod C"
            description = "Test mod C"
            authors = ["Test"]
            mod_id = "test.mod_c"
            version = "1.0.0"
        "#,
        );

        let meta_a = create_test_meta(
            r#"
            name = "Mod A"
            description = "Test mod A"
            authors = ["Test"]
            mod_id = "test.mod_a"
            version = "1.0.0"
        "#,
        );

        let meta_b = create_test_meta(
            r#"
            name = "Mod B"
            description = "Test mod B"
            authors = ["Test"]
            mod_id = "test.mod_b"
            version = "1.0.0"
        "#,
        );

        mods.insert("test.mod_c".to_string(), meta_c.clone());
        mods.insert("test.mod_a".to_string(), meta_a.clone());
        mods.insert("test.mod_b".to_string(), meta_b.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod_c".to_string(), ("test.mod_c.ztd".to_string(), meta_c));
        discovered.insert("test.mod_a".to_string(), ("test.mod_a.ztd".to_string(), meta_a));
        discovered.insert("test.mod_b".to_string(), ("test.mod_b.ztd".to_string(), meta_b));

        let resolver = DependencyResolver::new(mods, &discovered);
        let pure_legacy: &[(String, PathBuf)] = &[];
        let result = resolver.resolve_order(&[], &[], pure_legacy);

        // Should be sorted alphabetically
        assert_eq!(result.order, vec!["test.mod_a", "test.mod_b", "test.mod_c"]);
        assert!(result.warnings.is_empty());
    }

    // ============================================================================
    // Tests for pure_legacy_in_mods parameter
    // ============================================================================

    #[test]
    fn test_pure_legacy_archives_added_to_order() {
        // Resolver with no OpenZT mods
        let discovered = HashMap::new();
        let resolver = DependencyResolver::new(HashMap::new(), &discovered);

        // Pure legacy archives in /mods/
        let pure_legacy: Vec<(String, PathBuf)> = vec![
            ("legacy_c.ztd".to_string(), PathBuf::from("./mods/legacy_c.ztd")),
            ("legacy_a.ztd".to_string(), PathBuf::from("./mods/legacy_a.ztd")),
            ("legacy_b.ztd".to_string(), PathBuf::from("./mods/legacy_b.ztd")),
        ];

        let result = resolver.resolve_order(&[], &[], &pure_legacy);

        // Should have all 3 legacy archives, sorted alphabetically
        assert_eq!(result.order.len(), 3);
        assert_eq!(result.order, vec!["legacy_a.ztd", "legacy_b.ztd", "legacy_c.ztd"]);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_mixed_order_with_mods_and_pure_legacy() {
        let mut mods = HashMap::new();

        // Create an OpenZT mod
        let meta_mod = create_test_meta(
            r#"
            name = "Test Mod"
            description = "Test mod"
            authors = ["Test"]
            mod_id = "test.mod"
            version = "1.0.0"
        "#,
        );

        mods.insert("test.mod".to_string(), meta_mod.clone());

        let mut discovered = HashMap::new();
        discovered.insert("test.mod".to_string(), ("test.mod.ztd".to_string(), meta_mod));

        let resolver = DependencyResolver::new(mods, &discovered);

        // Mixed existing order with both mod_id and ZTD filename
        let existing_order = vec!["legacy_a.ztd".to_string(), "test.mod".to_string(), "legacy_b.ztd".to_string()];

        let pure_legacy: Vec<(String, PathBuf)> = vec![
            ("legacy_a.ztd".to_string(), PathBuf::from("./mods/legacy_a.ztd")),
            ("legacy_b.ztd".to_string(), PathBuf::from("./mods/legacy_b.ztd")),
        ];

        let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

        // Order should be preserved
        assert_eq!(result.order.len(), 3);
        assert_eq!(result.order, vec!["legacy_a.ztd", "test.mod", "legacy_b.ztd"]);
    }

    #[test]
    fn test_pure_legacy_inserted_at_position_zero() {
        let mut mods = HashMap::new();

        // Existing mod
        let meta_existing = create_test_meta(
            r#"
            name = "Existing Mod"
            description = "Existing mod"
            authors = ["Test"]
            mod_id = "existing.mod"
            version = "1.0.0"
        "#,
        );

        mods.insert("existing.mod".to_string(), meta_existing.clone());

        let mut discovered = HashMap::new();
        discovered.insert("existing.mod".to_string(), ("existing.mod.ztd".to_string(), meta_existing));

        let resolver = DependencyResolver::new(mods, &discovered);

        // Existing order with one mod
        let existing_order = vec!["existing.mod".to_string()];

        // New pure legacy archives (not in existing order)
        let pure_legacy: Vec<(String, PathBuf)> = vec![
            ("zzz.ztd".to_string(), PathBuf::from("./mods/zzz.ztd")),
            ("aaa.ztd".to_string(), PathBuf::from("./mods/aaa.ztd")),
        ];

        let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

        // Should have 3 entries: pure legacy archives at start (alphabetically), then existing mod
        assert_eq!(result.order.len(), 3);
        assert_eq!(result.order[0], "aaa.ztd");
        assert_eq!(result.order[1], "zzz.ztd");
        assert_eq!(result.order[2], "existing.mod");
    }

    #[test]
    fn test_disabled_pure_legacy_stays_in_order() {
        let discovered = HashMap::new();
        let resolver = DependencyResolver::new(HashMap::new(), &discovered);

        // Existing order has a disabled pure legacy archive
        let existing_order = vec!["disabled_legacy.ztd".to_string()];
        let disabled = vec!["disabled_legacy.ztd".to_string()];

        let pure_legacy: Vec<(String, PathBuf)> = vec![("disabled_legacy.ztd".to_string(), PathBuf::from("./mods/disabled_legacy.ztd"))];

        let result = resolver.resolve_order(&existing_order, &disabled, &pure_legacy);

        // Disabled pure legacy should stay in order
        assert_eq!(result.order.len(), 1);
        assert_eq!(result.order[0], "disabled_legacy.ztd");
    }

    #[test]
    fn test_pure_legacy_not_filtered_when_missing() {
        // Test that missing pure legacy archives stay in the order (not filtered out)
        let discovered = HashMap::new();
        let resolver = DependencyResolver::new(HashMap::new(), &discovered);

        // Existing order references a pure legacy archive that doesn't exist on disk
        let existing_order = vec!["missing_legacy.ztd".to_string()];
        let pure_legacy: &[(String, PathBuf)] = &[]; // Not discovered

        let result = resolver.resolve_order(&existing_order, &[], pure_legacy);

        // Missing entry should stay in order (error is logged but not removed)
        assert_eq!(result.order.len(), 1);
        assert_eq!(result.order[0], "missing_legacy.ztd");
    }
}
