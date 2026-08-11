# Behavior-test replacement gate

`manifest.json` maps removed source-text and prompt-text assertions to executable
behavior boundaries. `run.ps1` runs every replacement command and fails on the
first behavior regression. The manifest is intentionally declarative; it does
not scan production source or treat function names and prompt wording as proof
of behavior.

Run with:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tests/equivalence/test_quality/run.ps1
```
