# npm support appeal — release unscoped name `atref`

**Status:** draft, ready to submit (2026-06-04)
**Where to submit:** <https://www.npmjs.com/support> → "Open a support ticket" (or email support@npmjs.com from the account address)
**Account:** `juanjofuchs`

---

## Subject

Request to publish unscoped package `atref` — blocked by similarity filter (false positive vs `stres`)

## Body

Hi npm Support,

I'm trying to publish a new open-source package named `atref` from my account
(`juanjofuchs`), but `npm publish` is rejected with:

> 403 Forbidden - PUT https://registry.npmjs.org/atref - Package name too
> similar to existing package stres

`atref` is an actively developed, MIT-licensed CLI tool — a global
file-reference picker (press a keyboard chord in any text field, fuzzy-pick a
file, and insert its path or `[[wikilink]]`). It is already published under the
same name on crates.io: <https://crates.io/crates/atref>. The public GitHub
repository is at <https://github.com/juanjofuchs/atref>.

The package it's being flagged against, `stres`
(<https://www.npmjs.com/package/stres>), appears to be an unmaintained
placeholder: its description is the unmodified default `create-next-app`
boilerplate text, and there is no functional or semantic overlap with `atref`.
The names differ by two characters and serve entirely unrelated purposes, so I
believe this is a false positive from the name-similarity filter.

Could you please allow me to publish the unscoped package `atref` under the
`juanjofuchs` account? I'm happy to provide any additional verification.

Thank you,
Juan José Fuchs (npm: `juanjofuchs`)

---

## Context for future me

- crates.io reservation succeeded the same day (`atref v0.0.0`), so the name is
  permanently held there regardless of the npm outcome.
- The unscoped npm name is frozen for everyone by this same filter, so there is
  no squatting risk while the appeal is pending.
- Fallback if the appeal is denied: scoped package `@juanjofuchs/atref`
  (`npx @juanjofuchs/atref`), or revisit at spec 002 (packaging).
- A built, dry-run-clean placeholder package is staged in `npm/` — ready to
  `npm publish npm/` the moment npm clears the name.
