#!/usr/bin/env python3
"""
Detour Test Automation Script for OpenZT

This script automates the testing of OpenZT function detours using the
openzt-instance-manager CLI. It parses detours from generated.rs, reads
existing test results from CSV, distributes testing across multiple
endpoints, and records results.

Result codes:
    0 = Not tested / not yet attempted
    1 = Crashed (detour caused game crash)
    2 = Success (detour called and exited cleanly)
    3 = In use (actively used in openzt source code)

Usage:
    python3 detour_test_automation.py [options]

Options:
    --config PATH       Custom config file path
    --detour NAME       Test single detour (for debugging)
    --resume            Continue from existing CSV (default)
    --fresh             Start fresh, ignore existing results
    --dry-run           Show what would be tested without executing
"""

import argparse
import base64
import concurrent.futures
import csv
import datetime
import json
import re
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

# Try to use standard library tomllib (Python 3.11+), fall back to tomli
try:
    import tomllib
    def load_toml(path):
        with open(path, 'rb') as f:
            return tomllib.load(f)
except ImportError:
    try:
        import tomli
        def load_toml(path):
            with open(path, 'rb') as f:
                return tomli.load(f)
    except ImportError:
        print("Error: No TOML library available.")
        print("Install tomli: pip install tomli")
        sys.exit(1)


def http_request(url: str, method: str = 'GET', data: Optional[dict] = None, timeout: int = 30) -> Tuple[int, dict]:
    """
    Make an HTTP request using urllib.

    Returns:
        Tuple of (status_code, response_data)
    """
    headers = {'Content-Type': 'application/json'}

    if data is not None:
        body = json.dumps(data).encode('utf-8')
        req = urllib.request.Request(url, data=body, headers=headers, method=method)
    else:
        req = urllib.request.Request(url, headers=headers, method=method)

    try:
        with urllib.request.urlopen(req, timeout=timeout) as response:
            response_data = json.load(response)
            return response.status, response_data
    except urllib.error.HTTPError as e:
        # Try to read error response
        try:
            error_data = json.load(e)
            return e.code, error_data
        except:
            return e.code, {'error': str(e)}
    except urllib.error.URLError as e:
        return 0, {'error': str(e)}
    except Exception as e:
        return 0, {'error': str(e)}


@dataclass
class Endpoint:
    """Represents an openzt-instance-manager endpoint"""
    url: str
    max_instances: int
    active_instances: int = 0

    def can_allocate(self) -> bool:
        return self.active_instances < self.max_instances

    def allocate(self) -> bool:
        if self.can_allocate():
            self.active_instances += 1
            return True
        return False

    def release(self):
        self.active_instances = max(0, self.active_instances - 1)


@dataclass
class TestResult:
    """Result of a detour test"""
    detour_name: str
    result: int  # 0=not tested, 1=crashed, 2=success, 3=in use (actively used in openzt)
    timestamp: str
    instance_id: str = ""

    @staticmethod
    def from_csv_row(row: Dict[str, str]) -> 'TestResult':
        return TestResult(
            detour_name=row['detour_name'],
            result=int(row['result']),
            timestamp=row.get('last_test_timestamp', ''),
            instance_id=row.get('instance_id', '')
        )

    def to_csv_row(self) -> Dict[str, str]:
        return {
            'detour_name': self.detour_name,
            'result': str(self.result),
            'last_test_timestamp': self.timestamp,
            'instance_id': self.instance_id
        }

    @staticmethod
    def result_code_to_string(code: int) -> str:
        """Convert result code to human-readable string"""
        codes = {
            0: "not tested",
            1: "crashed",
            2: "success",
            3: "in use"
        }
        return codes.get(code, f"unknown({code})")


@dataclass
class Config:
    """Configuration from TOML file"""
    max_parallel_instances: int
    dll_path: Path
    test_script_path: Path
    results_csv_path: Path
    endpoints: List[Endpoint]
    test_timeout_seconds: int
    poll_interval_seconds: int

    @staticmethod
    def from_file(path: Path) -> 'Config':
        data = load_toml(path)
        testing = data['testing']
        execution = data['execution']

        endpoints = [
            Endpoint(url=e['url'], max_instances=e['max_instances'])
            for e in data['endpoints']
        ]

        return Config(
            max_parallel_instances=testing['max_parallel_instances'],
            dll_path=Path(testing['dll_path']),
            test_script_path=Path(testing['test_script_path']),
            results_csv_path=Path(testing['results_csv_path']),
            endpoints=endpoints,
            test_timeout_seconds=execution['test_timeout_seconds'],
            poll_interval_seconds=execution['poll_interval_seconds']
        )


class DetourTestAutomation:
    """Main automation class for detour testing"""

    def __init__(self, config: Config, dry_run: bool = False):
        self.config = config
        self.dry_run = dry_run
        self.results: Dict[str, TestResult] = {}

    def parse_detours_from_generated_rs(self, file_path: Path) -> Set[str]:
        """Extract all detour names from generated.rs"""
        pattern = r'#\[cfg_attr\(feature = "detour-validation", validate_detour\("([^"]+)"\)\)\]'
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
            detours = set(re.findall(pattern, content))
        return detours

    def parse_active_detours_from_source(self, source_dir: Path) -> Set[str]:
        """
        Parse openzt source code to find actively used detours.

        Scans for #[detour(...)] patterns and their corresponding
        openzt_detour::generated::module::FUNCTION imports to construct
        full detour names (module/function).

        Returns:
            Set of detour names that are actively used in openzt
        """
        active_detours = set()

        # Find all Rust files in openzt/src
        rust_files = list(source_dir.rglob('*.rs'))

        for rust_file in rust_files:
            try:
                with open(rust_file, 'r', encoding='utf-8') as f:
                    content = f.read()

                # Find all #[detour(FUNCTION_NAME)] patterns
                detour_function_pattern = r'#\[detour\(([A-Z_][A-Z0-9_]*)\)\]'
                detour_functions = re.findall(detour_function_pattern, content)

                if not detour_functions:
                    continue

                # Find all openzt_detour imports to determine module names
                # Pattern: use openzt_detour::generated::module::{FUNCTION1, FUNCTION2, ...};
                import_pattern = r'use\s+openzt_detour::generated::([a-z_][a-z0-9_]*)::\{([^}]+)\}'
                imports = re.findall(import_pattern, content)

                # Build a map of function name -> module name
                fn_to_module = {}
                for module, functions_str in imports:
                    # Extract function names (handle trailing commas, whitespace)
                    functions = [f.strip().rstrip(',') for f in functions_str.split(',')]
                    for func in functions:
                        if func:  # Skip empty strings
                            fn_to_module[func] = module

                # Also check for single function imports: use openzt_detour::generated::module::FUNCTION;
                single_import_pattern = r'use\s+openzt_detour::generated::([a-z_][a-z0-9_]*)::([A-Z_][A-Z0-9_]*)'
                single_imports = re.findall(single_import_pattern, content)
                for module, func in single_imports:
                    fn_to_module[func] = module

                # Match detour functions to their modules
                for func in detour_functions:
                    if func in fn_to_module:
                        module = fn_to_module[func]
                        detour_name = f"{module}/{func.lower()}"
                        active_detours.add(detour_name)
                    else:
                        # Function not found in imports - might be in a #[detour_mod]
                        # Skip for now, these would need more complex parsing
                        pass

            except Exception as e:
                print(f"Warning: Error parsing {rust_file}: {e}")
                continue

        return active_detours

    def load_results_from_csv(self) -> Dict[str, TestResult]:
        """Load existing test results from CSV"""
        results = {}
        csv_path = self.config.results_csv_path

        if csv_path.exists():
            with open(csv_path, 'r', newline='', encoding='utf-8') as f:
                reader = csv.DictReader(f)
                for row in reader:
                    result = TestResult.from_csv_row(row)
                    results[result.detour_name] = result

        return results

    def save_results_to_csv(self):
        """Save current results to CSV"""
        csv_path = self.config.results_csv_path
        csv_path.parent.mkdir(parents=True, exist_ok=True)

        with open(csv_path, 'w', newline='', encoding='utf-8') as f:
            fieldnames = ['detour_name', 'result', 'last_test_timestamp', 'instance_id']
            writer = csv.DictWriter(f, fieldnames=fieldnames)
            writer.writeheader()

            for result in self.results.values():
                writer.writerow(result.to_csv_row())

    def read_file_as_base64(self, path: Path) -> str:
        """Read file and return base64-encoded content"""
        with open(path, 'rb') as f:
            content = f.read()
        return base64.b64encode(content).decode('utf-8')

    def create_instance(self, endpoint: Endpoint, detour_name: str) -> Tuple[bool, str, Optional[str]]:
        """
        Create an instance with detour validation enabled.

        Returns:
            Tuple of (success, instance_id, error_message)
        """
        if self.dry_run:
            print(f"  [DRY RUN] Would create instance on {endpoint.url} for detour {detour_name}")
            return True, "dry-run-id", None

        try:
            # Read DLL and Lua script
            dll_b64 = self.read_file_as_base64(self.config.dll_path)
            script_b64 = self.read_file_as_base64(self.config.test_script_path)

            # Build request body
            request_body = {
                "openzt_dll": dll_b64,
                "config": {
                    "validate_detours": [detour_name]
                },
                "scripts": [
                    {
                        "filename": "auto_test_detours.lua",
                        "content": script_b64
                    }
                ]
            }

            # Send request
            status_code, data = http_request(
                f"{endpoint.url}/api/instances",
                method='POST',
                data=request_body,
                timeout=30
            )

            if status_code == 200:
                instance_id = data['instance_id']
                return True, instance_id, None
            else:
                error_msg = data.get('error', str(data)) if data else 'Unknown error'
                return False, "", f"HTTP {status_code}: {error_msg}"

        except Exception as e:
            return False, "", str(e)

    def get_instance_status(self, endpoint: Endpoint, instance_id: str) -> Tuple[bool, Optional[str], Optional[str]]:
        """
        Get instance status.

        Returns:
            Tuple of (success, status, error_message)
        """
        if self.dry_run:
            return True, "running", None

        try:
            status_code, data = http_request(
                f"{endpoint.url}/api/instances/{instance_id}",
                timeout=10
            )

            if status_code == 200:
                return True, data['status'], None
            else:
                error_msg = data.get('error', str(data)) if data else 'Unknown error'
                return False, None, f"HTTP {status_code}: {error_msg}"

        except Exception as e:
            return False, None, str(e)

    def get_detour_results(self, endpoint: Endpoint, instance_id: str) -> Tuple[bool, Optional[dict], Optional[str]]:
        """
        Get detour test results.

        Returns:
            Tuple of (success, results_dict, error_message)
        """
        if self.dry_run:
            return True, {"passed": True, "results": [{"name": "test", "called": True}]}, None

        try:
            status_code, data = http_request(
                f"{endpoint.url}/api/instances/{instance_id}/detour-results",
                timeout=10
            )

            if status_code == 200:
                return True, data, None
            else:
                error_msg = data.get('error', str(data)) if data else 'Unknown error'
                return False, None, f"HTTP {status_code}: {error_msg}"

        except Exception as e:
            return False, None, str(e)

    def delete_instance(self, endpoint: Endpoint, instance_id: str) -> bool:
        """Delete an instance"""
        if self.dry_run:
            print(f"  [DRY RUN] Would delete instance {instance_id}")
            return True

        try:
            status_code, _ = http_request(
                f"{endpoint.url}/api/instances/{instance_id}",
                method='DELETE',
                timeout=10
            )
            return status_code == 200 or status_code == 404
        except Exception:
            return False

    def wait_for_test_completion(self, endpoint: Endpoint, instance_id: str, detour_name: str) -> int:
        """
        Wait for test to complete and determine result.

        Returns:
            Result code: 0=not called, 1=crashed, 2=success
        """
        if self.dry_run:
            print(f"  [DRY RUN] Would wait for test completion for detour {detour_name}")
            return 2  # Assume success in dry run

        start_time = time.time()
        timeout = self.config.test_timeout_seconds
        interval = self.config.poll_interval_seconds

        while time.time() - start_time < timeout:
            # Check instance status
            success, status, err = self.get_instance_status(endpoint, instance_id)
            if not success:
                print(f"  Error getting instance status: {err}")
                time.sleep(interval)
                continue

            # If instance is in error state, it crashed
            if status == "error":
                print(f"  Instance {instance_id} crashed (status: error)")
                return 1  # Crashed

            # If instance stopped, check detour results
            if status == "stopped":
                success, results, err = self.get_detour_results(endpoint, instance_id)
                if success and results:
                    if results.get('passed', False):
                        # Check if our detour was called
                        for result in results.get('results', []):
                            if result.get('name') == detour_name and result.get('called', False):
                                print(f"  Detour {detour_name} was called successfully")
                                return 2  # Success

                        # Detour wasn't called
                        print(f"  Detour {detour_name} was not called")
                        return 0  # Not called
                    else:
                        print(f"  Detour validation failed: {results}")
                        return 0  # Not called
                else:
                    print(f"  Error getting detour results: {err}")
                    return 0  # Not called

            # Still running or creating, wait
            time.sleep(interval)

        # Timeout
        print(f"  Timeout waiting for detour {detour_name}")
        return 0  # Not called

    def test_detour(self, detour_name: str, endpoints: List[Endpoint]) -> TestResult:
        """
        Test a single detour.

        Args:
            detour_name: Name of the detour to test
            endpoints: List of available endpoints

        Returns:
            TestResult with outcome
        """
        # Find available endpoint
        endpoint = None
        for ep in endpoints:
            if ep.allocate():
                endpoint = ep
                break

        if endpoint is None:
            print(f"  No available endpoints for detour {detour_name}")
            return TestResult(
                detour_name=detour_name,
                result=0,
                timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00', 'Z'),
                instance_id=""
            )

        print(f"Testing detour: {detour_name} on {endpoint.url}")

        # Create instance
        success, instance_id, err = self.create_instance(endpoint, detour_name)
        if not success:
            print(f"  Failed to create instance: {err}")
            endpoint.release()
            return TestResult(
                detour_name=detour_name,
                result=0,
                timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00', 'Z'),
                instance_id=""
            )

        print(f"  Created instance: {instance_id}")

        # Wait for test completion
        result_code = self.wait_for_test_completion(endpoint, instance_id, detour_name)

        # Delete instance
        if self.delete_instance(endpoint, instance_id):
            print(f"  Deleted instance: {instance_id}")
        else:
            print(f"  Warning: Failed to delete instance {instance_id}")

        # Release endpoint
        endpoint.release()

        # Return result
        result_str = TestResult.result_code_to_string(result_code)
        print(f"  Result: {result_str}")

        return TestResult(
            detour_name=detour_name,
            result=result_code,
            timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00', 'Z'),
            instance_id=instance_id
        )

    def run_tests(self, detours: Set[str], fresh: bool = False, source_dir: Optional[Path] = None):
        """
        Run tests for all or specified detours.

        Args:
            detours: Set of detour names to test
            fresh: If True, ignore existing results
            source_dir: Path to openzt source code to find active detours
        """
        # Load existing results
        if not fresh:
            self.results = self.load_results_from_csv()
            print(f"Loaded {len(self.results)} existing results from CSV")
        else:
            self.results = {}
            print("Starting fresh (ignoring existing results)")

        # Parse active detours from source if provided
        active_detours = set()
        if source_dir and source_dir.exists():
            print(f"Parsing active detours from {source_dir}...")
            active_detours = self.parse_active_detours_from_source(source_dir)
            print(f"Found {len(active_detours)} actively used detours in openzt source")

            # Mark active detours as "in use" (result code 3)
            # This updates existing results or adds new ones
            timestamp = datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00', 'Z')
            for detour in active_detours:
                # Only update if not already marked as in use
                if detour in self.results:
                    if self.results[detour].result != 3:
                        # Update to "in use" but preserve timestamp if it exists
                        existing = self.results[detour]
                        self.results[detour] = TestResult(
                            detour_name=detour,
                            result=3,  # 3 = in use
                            timestamp=existing.timestamp if existing.timestamp else timestamp,
                            instance_id=existing.instance_id
                        )
                else:
                    # Add new entry for active detour
                    self.results[detour] = TestResult(
                        detour_name=detour,
                        result=3,  # 3 = in use
                        timestamp=timestamp,
                        instance_id=""
                    )

            # Save updated results with active detours
            self.save_results_to_csv()

        # Filter out already tested detours (including active detours)
        if not fresh:
            untested = detours - set(self.results.keys())
            print(f"Found {len(detours)} total detours, {len(untested)} untested (excluding {len(active_detours)} active)")
            detours_to_test = untested
        else:
            detours_to_test = detours
            print(f"Testing {len(detours)} detours")

        if self.dry_run:
            print("\n[DRY RUN] No actual tests will be executed\n")

        # Track overall progress
        total = len(detours_to_test)
        completed = 0
        passed = 0
        failed = 0
        crashed = 0
        in_use = len(active_detours)

        # Use ThreadPoolExecutor for parallel execution
        max_workers = min(self.config.max_parallel_instances, len(self.config.endpoints))

        with concurrent.futures.ThreadPoolExecutor(max_workers=max_workers) as executor:
            # Submit all test jobs
            future_to_detour = {}
            for detour in sorted(detours_to_test):
                future = executor.submit(self.test_detour, detour, self.config.endpoints)
                future_to_detour[future] = detour

            # Process completed tests
            for future in concurrent.futures.as_completed(future_to_detour):
                detour = future_to_detour[future]
                try:
                    result = future.result()
                    self.results[result.detour_name] = result
                    completed += 1

                    # Update counters
                    if result.result == 2:
                        passed += 1
                    elif result.result == 1:
                        crashed += 1
                    else:
                        failed += 1

                    # Save results after each test
                    self.save_results_to_csv()

                    # Print progress
                    print(f"\nProgress: {completed}/{total} | "
                          f"Passed: {passed} | Crashed: {crashed} | Not Called: {failed} | In Use: {in_use}\n")

                except Exception as e:
                    print(f"Error testing detour {detour}: {e}")
                    completed += 1
                    failed += 1

        # Print summary
        print("\n" + "="*60)
        print("TEST SUMMARY")
        print("="*60)
        print(f"Total detours: {len(detours)}")
        print(f"Tested this run: {completed}")
        print(f"Passed: {passed}")
        print(f"Crashed: {crashed}")
        print(f"Not Called: {failed}")
        print(f"In Use (active in openzt): {in_use}")
        print(f"Results saved to: {self.config.results_csv_path}")
        print("="*60)

        # List detours that need investigation
        print("\nDetours that crashed (may need fixing):")
        crashed_detours = [d for d, r in self.results.items() if r.result == 1]
        if crashed_detours:
            for detour in sorted(crashed_detours):
                print(f"  - {detour}")
        else:
            print("  None")

        print("\nDetours not called (may need specific game actions):")
        not_called = [d for d, r in self.results.items() if r.result == 0]
        if not_called:
            for detour in sorted(not_called)[:20]:  # Show first 20
                print(f"  - {detour}")
            if len(not_called) > 20:
                print(f"  ... and {len(not_called) - 20} more")
        else:
            print("  None")

        print("\nDetours actively used in openzt:")
        active_list = [d for d, r in self.results.items() if r.result == 3]
        if active_list:
            for detour in sorted(active_list)[:30]:  # Show first 30
                print(f"  - {detour}")
            if len(active_list) > 30:
                print(f"  ... and {len(active_list) - 30} more")
        else:
            print("  None")


def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(
        description="Automated detour testing for OpenZT",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument(
        '--config',
        type=Path,
        default=Path(__file__).parent / 'detour_test_config.toml',
        help='Path to configuration file'
    )
    parser.add_argument(
        '--detour',
        type=str,
        help='Test single detour (for debugging)'
    )
    parser.add_argument(
        '--resume',
        action='store_true',
        default=True,
        help='Continue from existing CSV (default)'
    )
    parser.add_argument(
        '--fresh',
        action='store_true',
        help='Start fresh, ignore existing results'
    )
    parser.add_argument(
        '--dry-run',
        action='store_true',
        help='Show what would be tested without executing'
    )

    args = parser.parse_args()

    # Load configuration
    if not args.config.exists():
        print(f"Error: Config file not found: {args.config}")
        sys.exit(1)

    config = Config.from_file(args.config)

    # Validate paths
    if not config.dll_path.exists():
        print(f"Error: DLL not found: {config.dll_path}")
        print("Please build OpenZT first with: ./openzt.bat build --release")
        sys.exit(1)

    if not config.test_script_path.exists():
        print(f"Error: Test script not found: {config.test_script_path}")
        sys.exit(1)

    # Create automation instance
    automation = DetourTestAutomation(config, dry_run=args.dry_run)

    # Parse detours
    generated_rs = Path(__file__).parent.parent.parent / 'openzt-detour' / 'src' / 'generated.rs'
    if not generated_rs.exists():
        print(f"Error: generated.rs not found: {generated_rs}")
        sys.exit(1)

    all_detours = automation.parse_detours_from_generated_rs(generated_rs)
    print(f"Found {len(all_detours)} detours in {generated_rs}")

    # Filter to single detour if specified
    if args.detour:
        if args.detour in all_detours:
            detours_to_test = {args.detour}
        else:
            print(f"Error: Detour not found: {args.detour}")
            print(f"Available detours: {', '.join(sorted(all_detours)[:10])}")
            sys.exit(1)
    else:
        detours_to_test = all_detours

    # Find openzt source directory for parsing active detours
    source_dir = Path(__file__).parent.parent / 'src'

    # Run tests
    fresh = args.fresh
    automation.run_tests(detours_to_test, fresh=fresh, source_dir=source_dir)


if __name__ == '__main__':
    main()
