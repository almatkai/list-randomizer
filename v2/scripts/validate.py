"""Full JSON Schema validation plus pinned byte hashes; requires jsonschema."""
import hashlib
import json
from pathlib import Path
from jsonschema import Draft202012Validator

root = Path(__file__).resolve().parent.parent
read = lambda path: json.loads((root / path).read_text())
schema = read("tests/fixtures/beta/extension-v2.schema.json")
Draft202012Validator.check_schema(schema)
manifest = read("extension.json")
Draft202012Validator(schema).validate(manifest)
assert manifest["id"] == "com.tezbar.native.list-randomizer"
assert manifest["replaces"] == ["raycast.list-randomizer"]
assert manifest["preferences"] == []
assert [command["name"] for command in manifest["commands"]] == ["index"]
assert {v["platform"] for v in manifest["verification"]} == {"macOS", "Windows"}
assert all(v["state"] == "not-run" for v in manifest["verification"])
provenance = read("PROVENANCE.json")
for group in ["original", "apiReference"]:
    for path, expected in provenance[group]["sha256"].items():
        assert hashlib.sha256((root / path).read_bytes()).hexdigest() == expected, path
icon = (root / provenance["icon"]["source"]).read_bytes()
assert len(icon) == 23003
assert hashlib.sha256(icon).hexdigest() == provenance["icon"]["sha256"]
assert icon == (root / "tests/fixtures/original/command-icon.png").read_bytes()
assert provenance["icon"]["hostDestination"] == "src/renderer/public/extensions/icons/list-randomizer.png"
print("Draft 2020-12 schema, metadata, byte-exact fixtures and original icon: pass")
