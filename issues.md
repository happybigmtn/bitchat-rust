# Curriculum and Codebase Issues

This document records issues found while aligning the Feynman walkthroughs with the actual codebase for a semester‑length curriculum. These are prioritized to unblock teaching, ensure accuracy, and improve coherence.

## Critical

- Duplicate directory nesting: walkthroughs were under `feynman/walkthroughs/feynman/walkthroughs/`. Resolved by flattening all walkthroughs into `feynman/walkthroughs/` and updating the Semester Plan and README.
- Out‑of‑sync indexes: `feynman/walkthroughs/README.md` references files that don’t exist or don’t match names (e.g., `02_config_management_walkthrough.md` vs existing `02_config_module_walkthrough.md`). Action: updated README to point to the curated plan.
- Inconsistent numbering and duplicates: topics appear multiple times with different numbers (e.g., Main Application: `04_...` and `06_...`; Session/Resilience duplicated at 47/49 and 48/55). Action: semester plan defines a single canonical order; future renames should follow that.
- Documentation inconsistency: `FEYNMAN_WALKTHROUGH_VERIFICATION_REPORT.md` references `src/utils/task_tracker.rs` which does not exist. Action: correct the report and any walkthroughs that inherited this path.
- Fabricated/overstated features in some walkthroughs (e.g., comprehensive auto‑scaling, advanced lock‑free suites) vs. actual implementation. Action: mark as “Future Work” and scope realistically in chapters.

## High Priority

- Unclear source of truth: multiple TOCs (`00_TABLE_OF_CONTENTS*.md`) with differing scopes. Action: `00_SEMESTER_PLAN.md` is the course canonical order; `00_TABLE_OF_CONTENTS.md` remains a raw inventory of all walkthroughs.
- Missing foundational chapters for teaching: computer architecture (memory/cache/CPU), crypto math (finite fields/ECC), probability & house edge, networking fundamentals (NAT, MTU, congestion), and distributed systems math (quorum, thresholds). Action: added new foundational chapters to kick off the course.
- Code paths vs docs drift: some walkthroughs use old module paths (e.g., anti‑cheat, config). Action: cross‑link to current paths in each updated chapter.

## Medium Priority

- `unwrap()`/`expect()` in non‑test code in several modules (e.g., validation/security paths). Good teaching moment; flag for refactor when demonstrating error handling. Action: track remediation tasks per module during course labs.
- Minor compilation warnings/loose ends in advanced transport/security code noted in earlier reports; verify during build labs and pin exact locations.
- Mixed implementation status: some chapters blend current and future plans without clear labels. Action: mandate an “Implementation Status” section in the prompt template; apply as we touch chapters.

## Organizational Improvements

- Establish a “single index” model: course plan (weeks/chapters) → links to detailed walkthroughs. Avoid mass renames during the semester to keep Git history stable.
- Add per‑chapter “Prerequisites, Learning Outcomes, Lab Exercise, Math Focus” to raise educational value (prompt updated).

## Next Actions

- Optionally flatten the nested `feynman/walkthroughs/feynman/walkthroughs` later; defer until after the semester to minimize disruption.
- Continue refreshing chapters to use real code paths and measured line counts. Keep “Future Work” clearly labeled.
