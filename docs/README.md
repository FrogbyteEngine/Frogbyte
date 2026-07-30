# Frogbyte Documentation

This directory is the repository's source of truth for project direction, milestone boundaries, architectural decisions, and engineering policies.

Documentation is kept in version control so that changes can be reviewed, discussed, and traced alongside the code they govern.

## Start here

| Document or directory | Purpose |
|---|---:|
| [`PROJECT_CHARTER.md`](PROJECT_CHARTER.md) | Purpose, vision, governance, current mission, and project boundaries |
| [`milestones/`](milestones/) | Bounded deliverables, non-goals, entry gates, and completion criteria | Medium |
| [`engineering/`](engineering/) | Testing, safety, benchmarking, dependency, CI, review, and versioning policies | Low to medium |

## Document hierarchy

Different documents answer different questions:

| Document type | Main question |
|---|---|
| Project charter | Why does the project exist, and what governs it? |
| Milestone | What must be delivered and validated now? |
| Engineering policy | What rules govern implementation and validation? |

## Status vocabulary

Documents should declare a status in front matter or near the title.

| Status | Meaning |
|---|---|
| `Draft` | Under development and not yet authoritative |
| `Proposed` | Ready for explicit review and decision |
| `Accepted` | Approved as the current authoritative direction or policy |
| `Active` | Currently being executed, usually for a milestone |
| `Completed` | Required work and completion evidence have been recorded |
| `Superseded` | Replaced by a newer document or decision |
| `Rejected` | Considered and deliberately not adopted |
| `Deprecated` | Still present for compatibility or history but should not guide new work |
| `Accepted` | does not mean immutable. A later decision may supersede an accepted document while preserving the historical record. |

