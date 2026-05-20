#!/usr/bin/env python3
"""
Add #[cfg_attr(feature = "detour-validation", validate_detour("module/function"))]
attributes to all FunctionDef constants in generated.rs that don't already have them.

Usage:
    python add_validation_macros.py src/generated.rs --dry-run
    python add_validation_macros.py src/generated.rs --in-place
    python add_validation_macros.py src/generated.rs --output src/generated_with_validations.rs
"""

import argparse
import re
import sys
from pathlib import Path


def screaming_to_snake(name: str) -> str:
    """Convert SCREAMING_SNAKE_CASE to lowercase snake_case."""
    return name.lower()


def has_validate_detour_attr(lines: list, idx: int) -> bool:
    """Check if a function already has a validate_detour attribute above it."""
    # Look backwards from current position for cfg_attr with validate_detour
    i = idx - 1
    while i >= 0:
        line = lines[i].strip()
        if line.startswith("#[cfg_attr"):
            if "validate_detour" in line:
                return True
            # If it's a different cfg_attr, keep looking
        elif not line.startswith("#") and line != "":
            # Hit a non-attribute, non-empty line
            return False
        i -= 1
    return False


def add_validation_macros(input_file: str, output_file: str = None, in_place: bool = False, dry_run: bool = False) -> int:
    """
    Add validate_detour attributes to all FunctionDef constants that don't already have them.

    Returns the number of macros added.
    """
    input_path = Path(input_file)

    if not input_path.exists():
        print(f"Error: Input file '{input_file}' not found", file=sys.stderr)
        return 0

    with open(input_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    output_lines = []
    current_module = None
    macros_added = 0
    line_number = 0

    function_pattern = re.compile(r'pub\s+const\s+(\w+):\s+FunctionDef<')
    module_pattern = re.compile(r'pub\s+mod\s+(\w+)\s+\{')

    for line in lines:
        line_number += 1

        # Check for module declaration
        module_match = module_pattern.match(line.strip())
        if module_match:
            current_module = module_match.group(1)
            output_lines.append(line)
            continue

        # Check for function definition
        stripped_line = line.strip()
        function_match = function_pattern.match(stripped_line)
        if function_match and current_module:
            function_name = function_match.group(1)

            # Get the leading whitespace (indentation) from the function line
            indent_match = re.match(r'^(\s*)', line)
            indent = indent_match.group(1) if indent_match else ''

            # Check if it already has a validate_detour attribute
            if not has_validate_detour_attr(output_lines, len(output_lines)):
                # Generate the attribute with proper indentation
                macro_line = f'{indent}#[cfg_attr(feature = "detour-validation", validate_detour("{current_module}/{screaming_to_snake(function_name)}"))]\n'
                output_lines.append(macro_line)
                macros_added += 1
                if dry_run and macros_added <= 20:
                    print(f"Would add at line {line_number}: {macro_line.strip()}")
                elif macros_added % 100 == 0:
                    print(f"Added {macros_added} macros so far...", file=sys.stderr)

            output_lines.append(line)
        else:
            output_lines.append(line)

    if dry_run:
        print(f"\nTotal macros that would be added: {macros_added}")
        return macros_added

    # Write output
    if in_place:
        # Create backup
        backup_path = input_path.with_suffix(input_path.suffix + '.bak')
        with open(backup_path, 'w', encoding='utf-8') as f:
            f.writelines(lines)
        print(f"Backup created: {backup_path}")

        with open(input_path, 'w', encoding='utf-8') as f:
            f.writelines(output_lines)
        print(f"Modified {input_file} in place")
    elif output_file:
        output_path = Path(output_file)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, 'w', encoding='utf-8') as f:
            f.writelines(output_lines)
        print(f"Wrote output to: {output_file}")
    else:
        print("Error: Must specify --in-place or --output", file=sys.stderr)
        return 0

    print(f"Total macros added: {macros_added}")
    return macros_added


def main():
    parser = argparse.ArgumentParser(
        description='Add validate_detour macros to FunctionDef constants in generated.rs',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Dry run to see what changes
  python add_validation_macros.py src/generated.rs --dry-run

  # Apply changes (creates backup)
  python add_validation_macros.py src/generated.rs --in-place

  # Write to new file
  python add_validation_macros.py src/generated.rs --output src/generated_with_validations.rs
        """
    )

    parser.add_argument('input_file', help='Path to generated.rs file')
    parser.add_argument('--in-place', '-i', action='store_true',
                        help='Modify file in place (creates .bak backup)')
    parser.add_argument('--output', '-o', help='Write to a different file')
    parser.add_argument('--dry-run', '-d', action='store_true',
                        help='Show what would change without writing')

    args = parser.parse_args()

    if not args.in_place and not args.output and not args.dry_run:
        parser.print_help()
        print("\nError: Must specify one of --in-place, --output, or --dry-run", file=sys.stderr)
        sys.exit(1)

    if args.in_place and args.output:
        print("Error: Cannot specify both --in-place and --output", file=sys.stderr)
        sys.exit(1)

    add_validation_macros(args.input_file, args.output, args.in_place, args.dry_run)


if __name__ == '__main__':
    main()
