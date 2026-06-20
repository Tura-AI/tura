#!/usr/bin/env python3
"""Validate backend Rust test layout and Cargo feature gates."""

from __future__ import annotations

import re
import sys
from pathlib import Path


SCAN_ROOTS = ("crates", "commands", "agents", "personas")
GATED_TEST_DIRS = {
    "tests/business/": "business-tests",
    "tests/os_testing/": "os-tests",
    "tests/performance/": "performance-tests",
    "tests/live/": "live-tests",
    "tests/benchmark/": "benchmark-tests",
}
TYPED_TEST_DIRS = ("business", "os_testing", "performance", "live", "release", "benchmark")
FORBIDDEN_TEST_DIRS = ("tests/e2e/",)
WORKSPACE_BENCHMARK_DIRS = {
    "bug-fix",
    "commands",
    "daily-ops",
    "frontend-playwright",
    "lib",
    "media-internet",
    "media-presentation",
    "project-rebuild-refactor",
    "tooling",
    "tui",
}


def normalize(path: str | Path) -> str:
    return str(path).replace("\\", "/")


def read_package_name(cargo_toml: Path) -> str:
    for line in cargo_toml.read_text(encoding="utf-8").splitlines():
        match = re.match(r'\s*name\s*=\s*"([^"]+)"', line)
        if match:
            return match.group(1)
    raise ValueError(f"could not find package name in {cargo_toml}")


def parse_test_targets(cargo_toml: Path) -> list[dict[str, object]]:
    targets: list[dict[str, object]] = []
    current: dict[str, object] | None = None

    for raw_line in cargo_toml.read_text(encoding="utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line == "[[test]]":
            if current is not None:
                targets.append(current)
            current = {}
            continue
        if line.startswith("["):
            if current is not None:
                targets.append(current)
                current = None
            continue
        if current is None or "=" not in line:
            continue

        key, value = [part.strip() for part in line.split("=", 1)]
        if key in {"name", "path"}:
            match = re.match(r'"([^"]*)"', value)
            if match:
                current[key] = match.group(1)
        elif key == "required-features":
            current[key] = re.findall(r'"([^"]+)"', value)

    if current is not None:
        targets.append(current)

    return targets


def backend_crate_roots(repo: Path) -> list[Path]:
    roots: list[Path] = []
    for scan_root in SCAN_ROOTS:
        root = repo / scan_root
        if not root.exists():
            continue
        for cargo_toml in root.rglob("Cargo.toml"):
            if "target" in cargo_toml.parts:
                continue
            roots.append(cargo_toml.parent)
    return sorted(roots)


def gated_test_files(crate_root: Path) -> list[Path]:
    files: list[Path] = []
    tests_root = crate_root / "tests"
    for directory in TYPED_TEST_DIRS:
        root = tests_root / directory
        if root.exists():
            files.extend(sorted(root.glob("*.rs")))
    return files


def module_dir_allowed(typed_root: Path, child: Path) -> bool:
    if child.name == "helpers":
        return True
    for suffix in ("", "_flow", "_e2e"):
        if (typed_root / f"{child.name}{suffix}.rs").exists():
            return True
    return False


def validate_typed_directory_shape(package: str, tests_root: Path, errors: list[str]) -> None:
    """Typed suites are peers under tests/; business/os_testing may have helper modules."""
    if not tests_root.exists():
        return

    for typed_name in TYPED_TEST_DIRS:
        typed_root = tests_root / typed_name
        if typed_root.exists() and not any(typed_root.rglob("*")):
            errors.append(
                f"{package}: {normalize(typed_root.relative_to(tests_root.parent))}/ "
                "is empty; remove empty typed test directories"
            )

        if typed_root.exists():
            for child in sorted(path for path in typed_root.iterdir() if path.is_dir()):
                if package == "workspace" and typed_name == "benchmark":
                    if child.name not in WORKSPACE_BENCHMARK_DIRS:
                        errors.append(
                            f"{package}: {normalize(child.relative_to(tests_root.parent))}/ "
                            "is not a known benchmark category"
                        )
                    continue
                if typed_name in {"business", "os_testing", "live"} and module_dir_allowed(typed_root, child):
                    continue
                errors.append(
                    f"{package}: {normalize(child.relative_to(tests_root.parent))}/ "
                    "is not allowed; tests/business, tests/os_testing, and tests/live allow helpers "
                    "or module directories tied to a top-level target, while "
                    "tests/performance, tests/release, and tests/benchmark "
                    "remain flat typed suites."
                )

        for nested_name in TYPED_TEST_DIRS:
            nested_root = typed_root / nested_name
            if nested_root.exists():
                errors.append(
                    f"{package}: {normalize(nested_root.relative_to(tests_root.parent))}/ "
                    "nests a typed suite under another typed suite; "
                    "tests/business, tests/os_testing, tests/performance, tests/live, "
                    "tests/release, and tests/benchmark must be peers"
                )


def expected_feature(relative_path: str) -> str | None:
    for prefix, feature in GATED_TEST_DIRS.items():
        if relative_path.startswith(prefix):
            return feature
    return None


def main() -> int:
    repo = Path(__file__).resolve().parents[2]
    errors: list[str] = []

    for crate_root in backend_crate_roots(repo):
        cargo_toml = crate_root / "Cargo.toml"
        package = read_package_name(cargo_toml)
        targets = parse_test_targets(cargo_toml)
        targets_by_path: dict[str, dict[str, object]] = {}

        for target in targets:
            path = target.get("path")
            if not isinstance(path, str):
                continue
            normalized = normalize(path)
            for forbidden in FORBIDDEN_TEST_DIRS:
                if normalized.startswith(forbidden):
                    errors.append(
                        f"{package}: {normalized} must not live under {forbidden}; "
                        "required non-network integration tests belong under tests/ "
                        "so cargo test can discover them, business workflows under "
                        "tests/business, process/OS workflows under tests/os_testing, "
                        "performance/stress under tests/performance, key/third-party "
                        "flows under tests/live, release binary checks under "
                        "tests/release, and scoring under tests/benchmark"
                    )
            targets_by_path[normalized] = target
            feature = expected_feature(normalized)
            if feature is None:
                continue
            required_features = target.get("required-features")
            if not isinstance(required_features, list) or feature not in required_features:
                errors.append(
                    f"{package}: {normalized} must declare "
                    f'required-features = ["{feature}"]'
                )

        for test_file in gated_test_files(crate_root):
            relative = normalize(test_file.relative_to(crate_root))
            if relative not in targets_by_path:
                errors.append(
                    f"{package}: {relative} must be declared as [[test]] in Cargo.toml "
                    "so the explicit backend test runners can select it"
                )

        tests_root = crate_root / "tests"
        validate_typed_directory_shape(package, tests_root, errors)
        for forbidden in FORBIDDEN_TEST_DIRS:
            forbidden_root = tests_root / forbidden.removeprefix("tests/").rstrip("/")
            if forbidden_root.exists():
                errors.append(
                    f"{package}: {normalize(forbidden_root.relative_to(crate_root))}/ "
                    "is not an allowed backend test directory"
                )

    workspace_tests_root = repo / "tests"
    validate_typed_directory_shape("workspace", workspace_tests_root, errors)
    workspace_e2e_root = workspace_tests_root / "e2e"
    if workspace_e2e_root.exists():
        errors.append(
            "workspace: tests/e2e/ is not an allowed test directory; "
            "required local E2E flows belong under tests/business, live flows "
            "under tests/live, release binary checks under tests/release, "
            "process/OS flows under tests/os_testing, performance under "
            "tests/performance, and scoring under tests/benchmark"
        )

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print("backend Rust test layout is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
