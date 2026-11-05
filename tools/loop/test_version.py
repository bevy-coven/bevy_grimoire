#!/usr/bin/env python3
"""`tools/loop/version.py` against a temporary manifest, lockfile and tag list.

Run: python3 tools/loop/test_version.py
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import version  # noqa: E402


def manifest(version_text: str) -> Path:
    path = Path(tempfile.mkdtemp()) / "Cargo.toml"
    path.write_text(
        "[package]\n"
        'name = "bevy_cauldron"\n'
        f'version = "{version_text}"\n'
        "\n"
        "[dependencies]\n"
        'bevy = "0.20.0"\n'
    )
    return path


def lock(version_text: str) -> Path:
    path = Path(tempfile.mkdtemp()) / "Cargo.lock"
    path.write_text(
        'version = 4\n'
        '\n'
        '[[package]]\n'
        'name = "bevy_cauldron"\n'
        f'version = "{version_text}"\n'
        'dependencies = [\n'
        ' "bevy",\n'
        ']\n'
        '\n'
        '[[package]]\n'
        'name = "bevy_ecs"\n'
        'version = "0.20.0"\n'
    )
    return path


class ThePolicy(unittest.TestCase):
    def test_the_next_version_rolls_minor_at_ten(self):
        """PATCH counts to 9 and then rolls: that is what stops it becoming a build counter."""
        self.assertEqual(version.next_version("0.2.5"), "0.2.6")
        self.assertEqual(version.next_version("0.2.8"), "0.2.9")
        self.assertEqual(version.next_version("0.2.9"), "0.3.0")
        self.assertEqual(version.next_version("0.0.0"), "0.0.1")

    def test_the_next_version_never_invents_a_one_point_oh(self):
        """At the ceiling it refuses and names whose decision a `1.0.0` is."""
        with self.assertRaises(version.Refusal) as caught:
            version.next_version("0.9.9")
        self.assertIn("1.0.0", str(caught.exception))
        self.assertIn("owner", str(caught.exception))

    def test_a_version_that_is_not_x_dot_y_dot_z_is_refused(self):
        for bad in ["0.2", "0.2.5-beta.1", "0.2.5+build7", "v0.2.5", "0.2.5.1", "two.1.0"]:
            with self.subTest(bad=bad), self.assertRaises(version.Refusal):
                version.parse(bad)

    def test_past_one_point_oh_is_the_owners_decision(self):
        with self.assertRaises(version.Refusal) as caught:
            version.next_version("1.0.0")
        self.assertIn("owner", str(caught.exception))


class TheCheck(unittest.TestCase):
    def test_agreeing_files_and_tags_pass(self):
        self.assertEqual(
            version.problems(manifest("0.2.6"), lock("0.2.6"), tags=["bevy_cauldron-v0.2.6"]), []
        )

    def test_a_patch_above_the_ceiling_is_refused_with_the_number_to_aim_for(self):
        """The failure this policy exists for: 0.2.278 is a build counter wearing a version."""
        found = version.problems(manifest("0.2.278"), lock("0.2.278"), tags=[])
        self.assertEqual(len(found), 1, found)
        self.assertIn("0.2.278", found[0])
        self.assertIn("0.3.0", found[0], "the message says which version it should have been")

    def test_a_minor_above_the_ceiling_is_refused_and_names_the_owner(self):
        found = version.problems(manifest("0.12.0"), lock("0.12.0"), tags=[])
        self.assertEqual(len(found), 1, found)
        self.assertIn("1.0.0", found[0])
        self.assertIn("owner", found[0])

    def test_the_manifest_and_the_lock_must_agree(self):
        found = version.problems(manifest("0.2.6"), lock("0.2.5"), tags=[])
        self.assertEqual(len(found), 1, found)
        self.assertIn("0.2.6", found[0])
        self.assertIn("0.2.5", found[0])

    def test_a_lock_with_no_entry_is_a_failure_rather_than_a_pass(self):
        path = Path(tempfile.mkdtemp()) / "Cargo.lock"
        path.write_text('version = 4\n\n[[package]]\nname = "bevy_ecs"\nversion = "0.20.0"\n')
        found = version.problems(manifest("0.2.6"), path, tags=[])
        self.assertEqual(len(found), 1, found)
        self.assertIn("bevy_cauldron", found[0])

    def test_a_tag_outside_the_ceiling_is_named(self):
        found = version.problems(
            manifest("0.2.6"), lock("0.2.6"), tags=["bevy_cauldron-v0.2.278"]
        )
        self.assertEqual(len(found), 1, found)
        self.assertIn("bevy_cauldron-v0.2.278", found[0])

    def test_a_tag_that_is_not_a_version_is_named(self):
        found = version.problems(
            manifest("0.2.6"), lock("0.2.6"), tags=["bevy_cauldron-v0.2.6-rc1"]
        )
        self.assertEqual(len(found), 1, found)
        self.assertIn("pre-release", found[0])

    def test_the_real_manifest_is_inside_the_policy(self):
        """The repository as it stands, so a bump that breaks the rule fails on main and not later."""
        self.assertEqual(version.problems(), [], "the editor's own version breaks its policy")


if __name__ == "__main__":
    unittest.main(verbosity=2)
