import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest

from normalize_cargo_chef_recipe import normalize_manifest, normalize_recipe


class RecipeEditionTests(unittest.TestCase):
    def test_inherited_editions_preserve_all_other_manifest_and_recipe_fields(self):
        recipe = {"skeleton": {"manifests": [
            {"relative_path": "Cargo.toml", "contents": '[workspace.package]\nedition = "2024"\n'},
            {"relative_path": "crates/sample/Cargo.toml", "contents": '''[package]
name = "sample"
edition.workspace = true
[lib]
path = "source.rs"
edition = "2024"
[[test]]
name = "caller"
edition = "2024"
harness = false
''', "targets": [{"path": "source.rs", "kind": "Lib"}]},
        ], "lock_file": "unchanged lock", "config_file": "unchanged config"}}
        original = copy.deepcopy(recipe)
        normalized, count = normalize_recipe(recipe)
        self.assertEqual(count, 2)
        self.assertEqual(recipe, original)
        expected = tomllib.loads(original["skeleton"]["manifests"][1]["contents"])
        del expected["lib"]["edition"]
        del expected["test"][0]["edition"]
        self.assertEqual(tomllib.loads(normalized["skeleton"]["manifests"][1]["contents"]), expected)
        normalized["skeleton"]["manifests"][1]["contents"] = original["skeleton"]["manifests"][1]["contents"]
        self.assertEqual(normalized, original)

    def test_multiline_strings_cannot_be_mistaken_for_target_settings(self):
        contents = '''[package]
edition = "2024"
description = """example
[lib]
edition = "2024"
"""
[lib]
path = "source.rs"
edition = "2024"
'''
        normalized, count = normalize_manifest(contents, None)
        self.assertEqual(count, 1)
        self.assertEqual(tomllib.loads(normalized)["package"], tomllib.loads(contents)["package"])
        self.assertNotIn("edition", tomllib.loads(normalized)["lib"])
        self.assertEqual(normalize_manifest(normalized, None), (normalized, 0))

    def test_implicit_2015_package_edition_remains_implicit(self):
        normalized, count = normalize_manifest('[package]\nname="old"\n[lib]\nedition="2015"\n', None)
        self.assertEqual(count, 1)
        self.assertEqual(tomllib.loads(normalized), {"package": {"name": "old"}, "lib": {}})

    def test_nonredundant_or_unresolved_edition_is_an_error(self):
        for contents in [
            '[package]\nedition="2024"\n[lib]\nedition="2018"\n',
            '[package]\nedition.workspace=true\n[lib]\nedition="2024"\n',
        ]:
            with self.assertRaisesRegex(ValueError, "nonredundant"):
                normalize_manifest(contents, None)

    def test_cli_leaves_original_recipe_unchanged_when_a_later_manifest_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "recipe.json"
            data = json.dumps({"skeleton": {"manifests": [
                {"relative_path": "Cargo.toml", "contents": '[package]\nedition="2024"\n[lib]\nedition="2024"\n'},
                {"relative_path": "bad/Cargo.toml", "contents": '[package]\nedition="2024"\n[lib]\nedition="2018"\n'},
            ]}})
            path.write_text(data)
            result = subprocess.run([sys.executable, str(Path(__file__).with_name("normalize_cargo_chef_recipe.py")), str(path)], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("bad/Cargo.toml", result.stderr)
            self.assertEqual(path.read_text(), data)


if __name__ == "__main__":
    unittest.main()
