"""Offline unit tests for the lng_* reuse rule in scripts/check-lang.py
(run: python3 -m unittest discover -s scripts -p 'test_lng_reuse.py').
scripts/check-lang.py runs the real check on the tree."""
import importlib.util
import os
import unittest

_SPEC = importlib.util.spec_from_file_location(
    'check_lang', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'check-lang.py'))
cl = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(cl)

VALUES = {
    'lng_copy': {'': 'Copy'},
    'lng_add_to_group': {'': 'Add to group'},
    'lng_chats': {'one': '{count} chat', 'other': '{count} chats'},
    'qll_unique': {'': 'Quill only'},
}


def problems(extra, baseline=(), homonyms=None):
    values = dict(VALUES, **extra)
    return cl.lng_reuse_problems(values, set(baseline), homonyms or {})


def kinds(lines):
    return [[kind, key.rstrip(':')] for kind, key, *_ in (line.split(' ', 2) for line in lines)]


class LngReuse(unittest.TestCase):
    def test_new_duplicate_fails_and_names_the_lng_key(self):
        found, _ = problems({'qll_copy': {'': 'Copy'}})
        self.assertEqual(kinds(found), [['LNG-REUSE', 'qll_copy']])
        self.assertIn('lng_copy', found[0])

    def test_case_and_trailing_period_still_match(self):
        found, _ = problems({'qll_add': {'': 'Add to Group.'}})
        self.assertEqual(kinds(found), [['LNG-REUSE', 'qll_add']])

    def test_baselined_duplicate_passes(self):
        found, kept = problems({'qll_copy': {'': 'Copy'}}, baseline=['qll_copy'])
        self.assertEqual(found, [])
        self.assertEqual(kept, {'qll_copy'})

    def test_stale_baseline_entry_fails_and_update_drops_it(self):
        found, kept = problems({}, baseline=['qll_gone'])
        self.assertEqual(kinds(found), [['LNG-REUSE-STALE', 'qll_gone']])
        self.assertEqual(kept, set())

    def test_update_never_adds_a_new_duplicate(self):
        _, kept = problems({'qll_copy': {'': 'Copy'}}, baseline=[])
        self.assertEqual(kept, set())

    def test_listed_homonym_passes_and_stale_homonym_fails(self):
        found, _ = problems({'qll_copy': {'': 'Copy'}}, homonyms={'qll_copy': 'noun', 'qll_old': 'x'})
        self.assertEqual(kinds(found), [['LNG-HOMONYM-STALE', 'qll_old']])

    def test_homonym_needs_a_reason(self):
        homonyms, found = cl.parse_homonyms(['qll_copy', 'qll_open adjective, lng_* is the verb'])
        self.assertEqual(kinds(found), [['LNG-HOMONYM', 'qll_copy']])
        self.assertEqual(homonyms['qll_open'], 'adjective, lng_* is the verb')

    def test_plural_matches_only_when_every_form_matches(self):
        same, _ = problems({'qll_chats': {'one': '{count} chat', 'other': '{count} chats'}})
        partial, _ = problems({'qll_chats': {'other': '{count} chats'}})
        self.assertEqual(kinds(same), [['LNG-REUSE', 'qll_chats']])
        self.assertEqual(partial, [])

    def test_lng_key_added_later_blames_nothing_new(self):
        found, _ = problems({'lng_quill': {'': 'Quill only'}})
        self.assertEqual(kinds(found), [['LNG-REUSE', 'qll_unique']])
        self.assertNotIn('new qll_', found[0])


if __name__ == '__main__':
    unittest.main()
