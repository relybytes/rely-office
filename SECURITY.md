# Security

rely-office reads files that users upload, so a crash, a hang or an unbounded allocation on a
crafted document is a security issue.

## Reporting

Use GitHub's private vulnerability reporting: **Security > Report a vulnerability** on this
repository, never a public issue. Include the version, the smallest input that shows the problem
(attached, not linked) and what happens. You get an acknowledgement within three working days.

## Supported versions

Only the latest released minor version receives fixes while the project is below 1.0.

## Scope

In scope: panics, infinite loops, memory or time far beyond the size of the input, reading outside
the input, and any way past `Limits`. Out of scope: documents that render wrongly, which are bugs to
report in the issue tracker.
