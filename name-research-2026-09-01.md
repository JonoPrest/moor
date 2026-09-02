# Name research for Moor

Checked: **2026-09-01 UTC**. This file is outside the repository and does not modify the project worktree.

## Bottom line

My recommendation is **Hawse** (`hawse`). It keeps the best part of Moor's existing story—durable attachment to an anchor—while the exact name was clear in all four checked catalogs. It is short, pronounceable, visually distinctive, and supports a coherent package family (`hawse-protocol`, `hawse-client-core`, `hawsed`). The main cost is obscurity: most people will need to hear the nautical story once.

If that feels too nautical, choose **Debur** for a concrete building/finishing metaphor, or **Elenk** for the most distinctive review-centered brand. **Weigh** is the strongest familiar English word, but npm is already occupied.

## What was checked

- 1,207 short candidates were generated and deduplicated; 608 had no crates.io sparse-index entry.
- 396 of the crates.io-free candidates also had no exact npm package, Homebrew formula/cask/alias/old-name, or Debian source/binary package collision.
- Homebrew coverage: 8,584 current core formulae and 7,719 current casks, including formula aliases/old names and old cask tokens.
- Debian coverage: 58,581 names in the Debian source archive plus binary package listings for bullseye, bookworm, trixie, forky, sid, and experimental across all architectures exposed by packages.debian.org. The source archive makes this deliberately conservative.
- npm coverage: exact unscoped registry metadata endpoint. npm can still reject a free exact name for confusing similarity or policy reasons.
- Manual web collision checks were performed for the leading candidates and obvious near-misses. This is not a trademark clearance.

**Legend:** “free/clear” means no exact catalog record at check time, not a reservation or guarantee of acceptance. Debian and Homebrew are curated packaging projects, not first-come name registries. Availability can change at any moment.

## Ranked shortlist

| # | Name | Say it | Why it fits | crates.io | Debian | Homebrew | npm | Web/search finding | Verdict |
|---:|---|---|---|---|---|---|---|---|---|
| 1 | `hawse` | HAWZ | The part of a ship's bow through which the anchor cable passes; preserves Moor's anchoring story while gaining a clean install name. | ✓ free | ✓ clear | ✓ clear | ✓ free | Low observed software collision; ordinary nautical and surname uses. | **Best overall** |
| 2 | `debur` | dee-BUR | A valid spelling of “deburr”: remove rough edges after fabrication, exactly what review does before work ships. | ✓ free | ✓ clear | ✓ clear | ✓ free | Low observed brand collision; an older EU robotics experiment used DEBUR descriptively. | **Best craft/building metaphor** |
| 3 | `elenk` | eh-LENK | A compact spelling derived from elench/elenchus: cross-examination or refutation through questioning. | ✓ free | ✓ clear | ✓ clear | ✓ free | No prominent exact-name software product surfaced in the spot-check. | **Best distinctive coinage** |
| 4 | `weigh` | WAY | To assess carefully, with a second nautical reading in “weigh anchor.” Short, familiar, and excellent aloud. | ✓ free | ✓ clear | ✓ clear | ✗ taken | Generic word with substantial search noise; npm exact name is occupied. | **Best plain-English idea; npm compromise** |
| 5 | `clews` | KLOOZ | Clews are attachment corners of a sail; the sound also evokes clues found during review. | ✓ free | ✓ clear | ✓ clear | ✓ free | No prominent software product surfaced; surname and boating-business uses exist. | **Best playful nautical option** |
| 6 | `crity` | KRIT-ee | Immediately evokes critique/critic and produces tidy family names such as crity-core and crityd. | ✓ free | ✓ clear | ✓ clear | ✓ free | An exact small software-company page exists, so legal/search clearance matters. | **Best review-forward coinage** |
| 7 | `qere` | KAY-ray / KEH-ree | A marginal reading note preserving a distinction between what is written and what should be read. | ✓ free | ✓ clear | ✓ clear | ✓ free | Low software collision, but QERE consumer electronics exist; culturally specific and pronunciation varies. | **Best scholarly annotation story** |
| 8 | `flaws` | FLAWZ | Review finds flaws before release. Completely legible and unusually registry-clean for an English word. | ✓ free | ✓ clear | ✓ clear | ✓ free | No exact-name software brand surfaced; generic term creates heavy search noise. | **Clearest function, harsher tone** |
| 9 | `burrs` | BURZ | The small rough edges left by making something; reviewers find them and deburring removes them. | ✓ free | ✓ clear | ✓ clear | ✓ free | No exact software brand surfaced; dominated by machining and tool results. | **Good finishing metaphor** |
| 10 | `thole` | THOHL | A pin or peg that keeps an oar located while still allowing motion—a surprisingly apt durable-anchor image. | ✓ free | ✓ clear | ✓ clear | ✓ free | No prominent software product surfaced; the word will need explanation. | **Clean but very obscure** |
| 11 | `cites` | SIGHTS | Comments cite exact blobs and lines; also sounds like sites/sights, which weakens spelling recall. | ✓ free | ✓ clear | ✓ clear | ✓ free | Generic scholarly term; high search noise but no prominent exact software brand surfaced. | **Good provenance angle** |
| 12 | `diple` | DIP-lee | An ancient marginal sign used to draw attention to noteworthy or dubious text. | ✓ free | ✓ clear | ✓ clear | ✓ free | Existing digital consultancy/corpus software and smartphone-microscope product make this a medium-risk choice. | **Great story, meaningful collisions** |

## Why Hawse wins

`hawse` is a real word, pronounced “haws.” It names the part of a ship's bow containing the hawseholes and can also mean the distance between bow and anchor. That gives the project a direct line from its current identity without falsely implying that it only finds defects. The system's core promise—comments remain connected to content while refs and diffs move—maps naturally to the cable passing from ship to anchor.

The command is comfortable (`hawse review create`, `hawse comment`, `hawse events --follow`), and the daemon can be `hawsed`. Searchability is much better than `moor`, `proof`, `peer`, or `weigh`. The only real weakness is discoverability of meaning, which is solvable with one sentence of product copy and a strong mark.

## Workspace crate-family check

For every shortlisted root, I also checked the 12 names implied by this repository's architecture: `-protocol`, `-protocol-fixtures`, `-review-core`, the `d` daemon, `-client-core`, `-client-wasm`, `-client-tauri`, `-client-tui`, `-cli`, `-mcp`, `-config`, and `-test-support`.

| Root | Family result on crates.io | Example daemon | Example core crate |
|---|---|---|---|
| `hawse` | ✓ all 12 free | `hawsed` | `hawse-review-core` |
| `debur` | ✓ all 12 free | `deburd` | `debur-review-core` |
| `elenk` | ✓ all 12 free | `elenkd` | `elenk-review-core` |
| `weigh` | ✓ all 12 free | `weighd` | `weigh-review-core` |
| `clews` | ✓ all 12 free | `clewsd` | `clews-review-core` |
| `crity` | ✓ all 12 free | `crityd` | `crity-review-core` |
| `qere` | ✓ all 12 free | `qered` | `qere-review-core` |
| `flaws` | ✓ all 12 free | `flawsd` | `flaws-review-core` |
| `burrs` | ✓ all 12 free | `burrsd` | `burrs-review-core` |
| `thole` | ✓ all 12 free | `tholed` | `thole-review-core` |
| `cites` | ✓ all 12 free | `citesd` | `cites-review-core` |
| `diple` | ✓ all 12 free | `dipled` | `diple-review-core` |

## Strong alternatives by desired tone

| Desired tone | Pick | Reason |
|---|---|---|
| Nautical + durable | `hawse` | Best continuation of the content-anchor concept; clean everywhere checked. |
| Craft + shipping | `debur` | Review removes the rough edges left by fabrication; plain action verb. |
| Intellectual + distinctive | `elenk` | Derived from cross-examination/refutation; very searchable and registry-clean. |
| Familiar English | `weigh` | Judgment plus nautical resonance; accept the npm collision and generic SEO. |
| Playful nautical | `clews` | Attachment points plus the sound of “clues”; clean everywhere checked. |
| Directly review-like | `crity` | Obvious critique association, but an exact small software company exists. |
| Editorial/scholarly | `qere` | A durable marginal reading note, but culturally specific and harder to pronounce. |
| Blunt quality tool | `flaws` | Immediately understood, though too adversarial for a collaborative product. |

## Attractive names I would reject

| Name | Why not |
|---|---|
| `moor` | Crates is free, but Homebrew's moor pager installs the same `moor` binary; Debian and npm also have exact packages. |
| `revit` | Crates is free, but Autodesk Revit is a major, long-established software product and npm is occupied. |
| `revu` | Crates is free, but Bluebeam Revu is established software and an open-source Tauri Git/agent review tool already uses `revu`. |
| `revie` | All four checked registries are clear, but Revie is already a well-reviewed Shopify review/marketing app. |
| `revly` | All four checked registries are clear, but Revly is already a software-review management platform. |
| `revis` | All four checked registries are clear, but REVIS is already the name of a Rust error-visualization tool/research project. |
| `dokim` | All four registries are clear, but KRINO Dokim is active review-ready BDD/QA software. |
| `krino` | All four registries are clear, but several current AI/decision products already use Krino. |
| `cense` | All four registries are clear, but current compliance, accounting, and AI software products use Cense. |
| `spexi` | All four registries are clear, but Spexi is an established drone imagery software/platform company. |
| `snagr` | All four registries are clear, but multiple exact-name snagging/property-inspection products exist. |
| `emend` | Crates is free and Debian/Homebrew are clear, but npm is occupied and an active developer migration CLI uses Emend. |
| `basan` | All four registries are clear, but Basan is an active agentic security product for developers. |
| `fiduc` | All four registries are clear, but Fiduc is an active fintech and mobile app. |
| `notio` | All four registries are clear, but several AI notes/business apps and a technology consultancy use Notio. |
| `redue` | All four registries are clear, but Redue is an existing routines/productivity app. |

## Longer project names with short CLI aliases

Only rows whose **project name** was free on crates.io are shown. Alias availability is reported separately because the binary alias need not itself be the crate name, but command/package collisions still matter.

| Project | CLI alias | Rationale | Project crates | Debian | Homebrew | npm | Alias crates | Alias Debian | Alias Homebrew | Alias npm |
|---|---|---|---|---|---|---|---|---|---|---|
| `anchorpoint` | `anc` | Comments remain attached to content | ✓ free | ✓ clear | ✓ clear | ✓ free | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `closeread` | `crd` | The careful reading that code review demands | ✓ free | ✓ clear | ✓ clear | ✓ free | ✗ taken | ✓ clear | ✓ clear | ✗ taken |
| `codeproof` | `cprf` | Proofing code before it ships | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✓ clear | ✓ clear | ✗ taken |
| `commentloom` | `cloom` | A place where comment threads are woven together | ✓ free | ✓ clear | ✓ clear | ✓ free | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `critiq` | `crit` | Distinct spelling of critique with a natural CLI alias | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✓ clear | ✗ formula | ✗ taken |
| `crosscheck` | `xchk` | Independent verification from another perspective | ✓ free | ✓ clear | ✓ clear | ✓ free | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fastened` | `fast` | Comments stay attached while code moves | ✓ free | ✓ clear | ✓ clear | ✓ free | ✗ taken | ✓ clear | ✓ clear | ✗ taken |
| `finecomb` | `fcomb` | A fine-tooth-comb inspection | ✓ free | ✓ clear | ✓ clear | ✓ free | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `galleyproof` | `gal` | The pre-publication artifact reviewed for corrections | ✓ free | ✓ clear | ✓ clear | ✓ free | ✗ taken | ✗ present | ✓ clear | ✗ taken |
| `hawsepipe` | `hawse` | The passage connecting ship and anchor cable | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `marginnote` | `mnote` | A durable comment attached beside the work | ✓ free | ✓ clear | ✗ cask | ✗ taken | ✗ taken | ✓ clear | ✓ clear | ✗ taken |
| `peerage` | `peer` | A playful home for peer review | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `plumbline` | `plumb` | Tests whether work is true | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✓ clear | ✓ clear | ✗ taken |
| `proofline` | `prfln` | A line inspected and proven before release | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `proofmark` | `pmk` | A mark certifying that something passed proof | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✗ present | ✓ clear | ✗ taken |
| `proofread` | `prdr` | The closest editorial analogue to code review | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `punchlist` | `plist` | The defects and finishing work found before handover | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✓ clear | ✓ clear | ✗ taken |
| `reviewboard` | `rbrd` | A panel and shared surface for review | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `reviewdeck` | `deck` | A shared surface for review work | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✗ present | ✗ formula | ✗ taken |
| `reviewforge` | `rforg` | Review improves work through heat and shaping | ✓ free | ✓ clear | ✓ clear | ✓ free | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `reviewit` | `revit` | Direct review-it blend; alias is catchy but conflicts with Autodesk Revit | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `reviewloom` | `rloom` | Threads of review woven into the code | ✓ free | ✓ clear | ✓ clear | ✓ free | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `secondlook` | `slk` | Plain-language description of review | ✓ free | ✓ clear | ✓ clear | ✓ free | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `secondpass` | `spass` | A deliberate second pass over work | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✗ present | ✓ clear | ✗ taken |
| `shipcheck` | `shchk` | The check immediately before work ships | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `soundingline` | `sndln` | A line used to measure hidden depth | ✓ free | ✓ clear | ✓ clear | ✓ free | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `soundings` | `snd` | Nautical measurement and a diagnostic check | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✗ present | ✓ clear | ✗ taken |
| `throughline` | `thru` | Continuity across commits and revisions | ✓ free | ✓ clear | ✓ clear | ✗ taken | ✗ taken | ✓ clear | ✓ clear | ✗ taken |

None of the longer forms beats `hawse`, `debur`, or `elenk`: the memorable aliases are generally already taken, while the clean aliases tend to feel cryptic.

## Complete crates.io-free table (608 names)

This is the broad option set requested, not 608 endorsements. “Ideation” is a lightweight thematic score from the generation pass: 5–6 = especially meaningful, 3–4 = plausible, 2 = exploratory coinage. The ranked shortlist and web-collision notes above carry much more weight.

| Name | Len | Theme | Ideation | Why it entered the set | crates.io | Debian | Homebrew | npm |
|---|---:|---|---:|---|---|---|---|---|
| `hawse` | 5 | anchoring | 3 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `debur` | 5 | craft | 5 | Building, fitting, sharpening, or finishing work | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `elenk` | 5 | inspection-rare | 6 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `weigh` | 5 | anchoring | 5 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `clews` | 5 | nautical-fastening | 5 | Specialist nautical fastening or anchor hardware | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crity` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `qere` | 4 | editorial-marks | 6 | Historic proofreading or textual-annotation mark | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `flaws` | 5 | finishing | 3 | A flaw or rough remnant found and removed during finishing | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `burrs` | 5 | finishing | 5 | A flaw or rough remnant found and removed during finishing | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `thole` | 5 | nautical-fastening | 5 | Specialist nautical fastening or anchor hardware | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cites` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `diple` | 5 | editorial-marks | 6 | Historic proofreading or textual-annotation mark | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `adher` | 5 | persistence | 6 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `basan` | 5 | inspection-rare | 6 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `dokim` | 5 | judgment | 6 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `elenx` | 5 | inspection-rare | 6 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fidu` | 4 | metrology | 6 | A fixed reference or pass/fail measurement | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fiduc` | 5 | metrology | 6 | A fixed reference or pass/fail measurement | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `grans` | 5 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `krino` | 5 | judgment | 6 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `naqd` | 4 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `naqid` | 5 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `nital` | 5 | finishing | 6 | A flaw or rough remnant found and removed during finishing | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pruef` | 5 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `recen` | 5 | inspection-rare | 6 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `relir` | 5 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revoi` | 5 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ryna` | 4 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sceaw` | 5 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `snagr` | 5 | finishing | 6 | A flaw or rough remnant found and removed during finishing | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `stead` | 5 | persistence | 6 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tarki` | 5 | languages | 6 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vettr` | 5 | inspection-rare | 6 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `btape` | 5 | approval-gate | 5 | A punch list, rating, or permission to proceed | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cense` | 5 | judgment | 5 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `clued` | 5 | persistence | 5 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `hunkr` | 5 | review-slang | 5 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `nitpi` | 5 | review-slang | 5 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `parly` | 5 | council | 5 | Peers meeting to discuss and reach judgment | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `plims` | 5 | approval-gate | 5 | A punch list, rating, or permission to proceed | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `prati` | 5 | approval-gate | 5 | A punch list, rating, or permission to proceed | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `redin` | 5 | review-slang | 5 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `redtg` | 5 | approval-gate | 5 | A punch list, rating, or permission to proceed | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revis` | 5 | languages | 5 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rived` | 5 | languages | 5 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scurf` | 5 | finishing | 5 | A flaw or rough remnant found and removed during finishing | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `stays` | 5 | persistence | 5 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `witns` | 5 | metrology | 5 | A fixed reference or pass/fail measurement | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ackd` | 4 | review-slang | 4 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `morsl` | 5 | cooking | 4 | Cooking, tasting, preparation, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `savur` | 5 | cooking-rare | 4 | Specialist tasting, sieving, or kitchen quality-control term | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tickt` | 5 | approval-gate | 4 | A punch list, rating, or permission to proceed | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `bluin` | 5 | metrology | 3 | A fixed reference or pass/fail measurement | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `calip` | 5 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `colum` | 5 | editorial | 3 | Editorial, manuscript, or proofing metaphor | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `copye` | 5 | editorial | 3 | Editorial, manuscript, or proofing metaphor | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crite` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crits` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `gybe` | 4 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `mancl` | 5 | editorial-marks | 3 | Historic proofreading or textual-annotation mark | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `moora` | 5 | anchoring | 3 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `moord` | 5 | anchoring | 3 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `moori` | 5 | anchoring | 3 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `moorn` | 5 | anchoring | 3 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `moory` | 5 | anchoring | 3 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pilcr` | 5 | editorial-marks | 3 | Historic proofreading or textual-annotation mark | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pinnd` | 5 | persistence | 3 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `redig` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `redue` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revie` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scrye` | 5 | inspection-rare | 3 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scryt` | 5 | inspection-rare | 3 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `selv` | 4 | material | 3 | Materials, structure, and durable workmanship | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sentr` | 5 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `signf` | 5 | approval-gate | 3 | A punch list, rating, or permission to proceed | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skewa` | 5 | cooking-rare | 3 | Specialist tasting, sieving, or kitchen quality-control term | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `spexi` | 5 | classical | 3 | Classical or cross-language root for seeing, testing, or judging | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ackra` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ackro` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ancor` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ancra` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ancro` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ankio` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ankra` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ankri` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ankry` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `annox` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `anota` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `anoti` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `anoto` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `antro` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `asaio` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `asay` | 4 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `asaya` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `asayo` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ausc` | 4 | diagnosis | 2 | Diagnosis, observability, or health-check metaphor | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cheka` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cheko` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `chekr` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cheku` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `chekz` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cheqa` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cheqi` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cheqo` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cheqx` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cheqy` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `chkio` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `chkup` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crino` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crint` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `criqa` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `criqi` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `criqo` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crita` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `criti` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crito` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `critq` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `critx` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `criva` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crive` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crivr` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crivu` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `crivy` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cruxa` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `cruxo` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `difex` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `diffa` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `difio` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `difix` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `difly` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `difra` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `difro` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `diftr` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `docko` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `dokra` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `dokro` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `essae` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `expoa` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `expoo` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `expor` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fita` | 4 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fiti` | 4 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fitly` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fitra` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fitro` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fixly` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fixra` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `fixro` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `forga` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `forgx` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `forjo` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `gazea` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `gazio` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `grada` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `gradi` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `grado` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `gradx` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `helmi` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `helmo` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `helmr` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `helmy` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `hona` | 4 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `honer` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `honex` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `honi` | 4 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `honio` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `hony` | 4 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `hunio` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `hunka` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `hunki` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `hunko` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `hunks` | 5 | code | 2 | Code review, diffs, patches, or Git objects | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `insee` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `insek` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `inspe` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `inspi` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `inspy` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `jigra` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `jigro` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `kedga` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `kedgo` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `keyda` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `kritq` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `krity` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `krivo` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `lensa` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `lensi` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `lensr` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `lensy` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `lokra` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `lokro` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `loku` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `looka` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `looki` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `looko` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `makio` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `marqa` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `marqi` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `merga` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `mergi` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `mergo` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `mergr` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `mergx` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `merio` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `merix` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `misen` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `misra` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `misro` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `mooro` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `morri` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `morry` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `notio` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `notly` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `notri` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `notro` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `notry` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `notza` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `notzo` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `okaya` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `okayo` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `okro` | 4 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `passa` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `passi` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `patix` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `patly` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `patri` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `patry` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `peeka` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `plati` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `platr` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `plumo` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `probi` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `probr` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `probu` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `probx` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `provo` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `provu` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `provx` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `prufa` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `prufi` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `prufo` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pruv` | 4 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pruva` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pruvo` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pruvy` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pulsa` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `pulsi` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `qlty` | 4 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `qltyx` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `quaio` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `quali` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `qualo` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `qualx` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `qualy` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `quaya` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `quayo` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `quayr` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `redly` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `redra` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `redro` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revar` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revdo` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revex` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revez` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revgo` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revia` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revin` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revly` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revok` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revon` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revor` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revos` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revra` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revro` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revry` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revum` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revvy` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revya` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revyn` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `revyo` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rewel` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rewia` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rewin` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rewio` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rewon` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rivi` | 4 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rivio` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rivix` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rivly` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rivro` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `rvwly` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `savra` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `savro` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scana` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scani` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scano` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scopa` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scopi` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scopo` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scopr` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scopx` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scor` | 4 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scora` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scori` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scoro` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `scorx` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sealo` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sealr` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `shipa` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `shipi` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `shipo` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `shipt` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `shipx` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `shipy` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifa` | 4 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifex` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifio` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifro` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifry` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifta` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifti` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifto` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `siftr` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `sifty` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skana` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skano` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skanr` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skany` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skopa` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skopi` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skopo` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `skopy` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `smito` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `speka` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `spekl` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `speko` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `spekr` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `speku` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `spekx` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `speqa` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `speqo` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `speqr` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `spexa` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `spexo` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `stamo` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `stamr` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tasta` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tasti` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tasto` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tastr` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tazta` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tazto` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `ticko` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `traco` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `traxa` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `traxo` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `trazi` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `trazr` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `trazy` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `truex` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `truio` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `truya` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `truyo` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `tryly` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `verin` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `verio` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `veru` | 4 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vetex` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `veti` | 4 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vetix` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vetly` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vetri` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vetu` | 4 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `veyro` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `viewa` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `viewi` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `viewo` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vistx` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `visty` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `voti` | 4 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `votra` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vuery` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyla` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyli` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vylo` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyra` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyri` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyua` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyui` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyuo` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyur` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `vyux` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `wheta` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `wheto` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `zesta` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `zesto` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `zestx` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✓ free |
| `abide` | 5 | persistence | 6 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `arris` | 5 | finishing | 6 | A flaw or rough remnant found and removed during finishing | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `bitts` | 5 | nautical-fastening | 6 | Specialist nautical fastening or anchor hardware | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `dwell` | 5 | persistence | 6 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `frap` | 4 | nautical-fastening | 6 | Specialist nautical fastening or anchor hardware | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `gono` | 4 | metrology | 6 | A fixed reference or pass/fail measurement | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `inher` | 5 | persistence | 6 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `nits` | 4 | review-slang | 6 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `plim` | 4 | approval-gate | 6 | A punch list, rating, or permission to proceed | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `probo` | 5 | judgment | 6 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `synod` | 5 | council | 6 | Peers meeting to discuss and reach judgment | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `tamis` | 5 | inspection-rare | 6 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `ackit` | 5 | review-slang | 5 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `annex` | 5 | persistence | 5 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `ausco` | 5 | inspection-rare | 5 | Close examination, screening, or diagnostic work | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `cerno` | 5 | judgment | 5 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `critr` | 5 | review-slang | 5 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `deye` | 4 | nautical-fastening | 5 | Specialist nautical fastening or anchor hardware | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `emend` | 5 | review | 5 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `essai` | 5 | classical | 5 | Classical or cross-language root for seeing, testing, or judging | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `gage` | 4 | metrology | 5 | A fixed reference or pass/fail measurement | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `infix` | 5 | persistence | 5 | Staying attached to the same underlying content | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `inkit` | 5 | review-slang | 5 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `kolla` | 5 | classical | 5 | Classical or cross-language root for seeing, testing, or judging | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `markr` | 5 | review-slang | 5 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `marl` | 4 | nautical-fastening | 5 | Specialist nautical fastening or anchor hardware | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `moor` | 4 | anchoring | 5 | Fastening, persistence, or content anchoring | ✓ free | ✗ present | ✗ formula | ✗ taken |
| `nitty` | 5 | review-slang | 5 | Developer-review vocabulary or compact critique coinage | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `palp` | 4 | inspection-rare | 5 | Close examination, screening, or diagnostic work | ✓ free | ✗ present | ✓ clear | ✓ free |
| `peer` | 4 | review | 5 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `prova` | 5 | classical | 5 | Classical or cross-language root for seeing, testing, or judging | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `pruf` | 4 | languages | 5 | Compact root for review, rereading, or scrutiny in another language | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `recto` | 5 | editorial | 5 | Editorial, manuscript, or proofing metaphor | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revu` | 4 | review | 5 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `sigl` | 4 | editorial-marks | 5 | Historic proofreading or textual-annotation mark | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `taste` | 5 | cooking | 5 | Cooking, tasting, preparation, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `tryst` | 5 | council | 5 | Peers meeting to discuss and reach judgment | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `witan` | 5 | judgment | 5 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `amend` | 5 | review | 4 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `aone` | 4 | approval-gate | 4 | A punch list, rating, or permission to proceed | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `barge` | 5 | shipping | 4 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `dhow` | 4 | shipping | 4 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `frete` | 5 | shipping | 4 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `ketch` | 5 | shipping | 4 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `opine` | 5 | review | 4 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `savor` | 5 | cooking | 4 | Cooking, tasting, preparation, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `scar` | 4 | finishing | 4 | A flaw or rough remnant found and removed during finishing | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `scrip` | 5 | metrology | 4 | A fixed reference or pass/fail measurement | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `skew` | 4 | finishing | 4 | A flaw or rough remnant found and removed during finishing | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `snub` | 4 | anchoring | 4 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `thing` | 5 | council | 4 | Peers meeting to discuss and reach judgment | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `trial` | 5 | review | 4 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `ack` | 3 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✗ present | ✗ formula | ✗ taken |
| `annot` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `apt` | 3 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✗ present | ✗ formula | ✗ taken |
| `baste` | 5 | cooking | 3 | Cooking, tasting, preparation, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `bevel` | 5 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `brad` | 4 | anchoring | 3 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `broil` | 5 | cooking | 3 | Cooking, tasting, preparation, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `broth` | 5 | cooking | 3 | Cooking, tasting, preparation, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `clear` | 5 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `coat` | 4 | craft | 3 | Building, fitting, sharpening, or finishing work | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `debug` | 5 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `deem` | 4 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `dockr` | 5 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `exact` | 5 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `fault` | 5 | diagnosis | 3 | Diagnosis, observability, or health-check metaphor | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `guide` | 5 | judgment | 3 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `jibe` | 4 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `judge` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `jury` | 4 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `knead` | 5 | cooking | 3 | Cooking, tasting, preparation, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `laden` | 5 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `loade` | 5 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `looks` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `mast` | 4 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `note` | 4 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✗ present | ✓ clear | ✗ taken |
| `oar` | 3 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✗ present | ✓ clear | ✗ taken |
| `peers` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `reads` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `reck` | 4 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `redos` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `resee` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revit` | 5 | review | 3 | Review, critique, discussion, or a second look | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `scale` | 5 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `scull` | 5 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `ships` | 5 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `steep` | 5 | cooking | 3 | Cooking, tasting, preparation, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `stern` | 5 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✗ formula | ✗ taken |
| `taut` | 4 | anchoring | 3 | Fastening, persistence, or content anchoring | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `tutor` | 5 | judgment | 3 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `wares` | 5 | shipping | 3 | Shipping, navigation, readiness, or a quality gate | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `weed` | 4 | inspection | 3 | Looking closely, measuring, testing, or finding defects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `wisk` | 4 | cooking-rare | 3 | Specialist tasting, sieving, or kitchen quality-control term | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `worth` | 5 | judgment | 3 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `yea` | 3 | judgment | 3 | Judgment, approval, standards, or peer collaboration | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `ache` | 4 | diagnosis | 2 | Diagnosis, observability, or health-check metaphor | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `anker` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `antra` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `bases` | 5 | code | 2 | Code review, diffs, patches, or Git objects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `basta` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `basto` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `binda` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `bindi` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `bindo` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `bindu` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `bondo` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `cheki` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `chkit` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `chkr` | 4 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `cough` | 5 | diagnosis | 2 | Diagnosis, observability, or health-check metaphor | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `crivo` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `diffi` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `docka` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `dockx` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✗ cask | ✓ free |
| `docky` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✗ present | ✓ clear | ✗ taken |
| `forgi` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `forgo` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `forgr` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `found` | 5 | finding | 2 | Discovery, signal, illumination, or bringing issues to light | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `gazer` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `grady` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `heads` | 5 | code | 2 | Code review, diffs, patches, or Git objects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `helma` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `hem` | 3 | material | 2 | Materials, structure, and durable workmanship | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `hint` | 4 | finding | 2 | Discovery, signal, illumination, or bringing issues to light | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `hunky` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `inspo` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `inspx` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `joint` | 5 | material | 2 | Materials, structure, and durable workmanship | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `kriti` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `krito` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `kriva` | 5 | coined-critique | 2 | Coined from critique/critic | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `labs` | 4 | diagnosis | 2 | Diagnosis, observability, or health-check metaphor | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `light` | 5 | finding | 2 | Discovery, signal, illumination, or bringing issues to light | ✓ free | ✗ present | ✓ clear | ✗ taken |
| `loka` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `looky` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `maka` | 4 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `makit` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `makra` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `makro` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `marka` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `marku` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `marqo` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `marqy` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `mergy` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `misa` | 4 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `misi` | 4 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `morra` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `notex` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `notix` | 5 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `notu` | 4 | coined-notes | 2 | Coined from note, mark, or annotation | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `oids` | 4 | code | 2 | Code review, diffs, patches, or Git objects | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `pachi` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✗ present | ✗ formula | ✗ taken |
| `passo` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `passy` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `patra` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `patro` | 5 | coined-diff | 2 | Coined from diff, hunk, patch, or merge | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `peeki` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `platy` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `proba` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `provi` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `pruve` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `quala` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `reda` | 4 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `redi` | 4 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `redy` | 4 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `reved` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revik` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revil` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revio` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revir` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revix` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revo` | 4 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revox` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `revup` | 5 | coined-review | 2 | Coined from review/revise | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `rivra` | 5 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `rivu` | 4 | coined-anchor | 2 | Coined from moor, anchor, bind, or rivet | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `scany` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `scent` | 5 | finding | 2 | Discovery, signal, illumination, or bringing issues to light | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `scopy` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `seala` | 5 | coined-quality | 2 | Coined around quality gates and approval | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `shipr` | 5 | coined-shipping | 2 | Coined from ship, dock, quay, or readiness | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `sifi` | 4 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `sifo` | 4 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `sifra` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `signs` | 5 | diagnosis | 2 | Diagnosis, observability, or health-check metaphor | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `smita` | 5 | coined-craft | 2 | Coined from forge, hone, fit, or making | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `spek` | 4 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✗ present | ✗ formula | ✗ taken |
| `spexy` | 5 | coined-inspection | 2 | Coined from inspect/spec/scope | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `teak` | 4 | material | 2 | Materials, structure, and durable workmanship | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `testa` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `testi` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `traci` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `tracr` | 5 | coined-sifting | 2 | Coined from sift, scan, trace, or probe | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `tryio` | 5 | coined-checking | 2 | Coined from check/test/assay | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `verix` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vetra` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vetro` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vety` | 4 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `veyra` | 5 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `viewr` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `visto` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vital` | 5 | diagnosis | 2 | Diagnosis, observability, or health-check metaphor | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vota` | 4 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `voto` | 4 | coined-vetting | 2 | Coined from vet, proof, or verification | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vuera` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vuero` | 5 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vyro` | 4 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `vyu` | 3 | coined-seeing | 2 | Coined from view/look/lens/gaze | ✓ free | ✓ clear | ✓ clear | ✗ taken |
| `zesty` | 5 | coined-cooking | 2 | Coined from taste, proof, mise, or the pass | ✓ free | ✓ clear | ✓ clear | ✗ taken |

## Method and reproducibility

- crates.io: exact lowercase names checked against the official sparse index paths. Rust keywords, standard/reserved crate names, and Windows device names were treated as reserved without a network lookup.
- Homebrew: official formula and cask JSON APIs; formula `name`, `full_name`, aliases, old names, cask token, and old cask tokens were included.
- Debian: the Sources API catalog plus the `allpackages` binary listings for the suites named above.
- npm: exact package metadata endpoint; HTTP 200 was treated as occupied and 404 as no current exact package.
- Scripts and raw caches are stored beside this report. Re-run `node check-registries.mjs`, `node gather-catalogs.mjs`, then `node build-report.mjs` to refresh.

## Source links

- [Cargo registry name restrictions](https://doc.rust-lang.org/cargo/reference/registry-index.html#name-restrictions)
- [crates.io sparse index](https://index.crates.io/)
- [Homebrew JSON API](https://formulae.brew.sh/docs/api/)
- [Homebrew formula naming guidance](https://docs.brew.sh/Formula-Cookbook#a-quick-word-on-naming)
- [Debian package-name policy](https://www.debian.org/doc/debian-policy/ch-controlfields.html#source)
- [Debian Sources API catalog](https://sources.debian.org/api/list/?format=json)
- [npm public registry API](https://github.com/npm/registry/blob/main/docs/REGISTRY-API.md)
- [npm package-name guidance](https://docs.npmjs.com/package-name-guidelines/)
- [Homebrew's existing `moor` formula](https://formulae.brew.sh/formula/moor)
- [Autodesk Revit](https://www.autodesk.com/products/revit/overview)
- [Existing `revu` Git/agent review tool](https://github.com/eddmann/revu)
- [Hawse definition](https://www.merriam-webster.com/dictionary/hawse)
- [Diple textual mark](https://en.wikipedia.org/wiki/Diple_%28textual_symbol%29)
- [Qere/ketiv scholarly overview](https://books.openbookpublishers.com/10.11647/obp.0207.08.pdf)
- [Elench definition](https://www.websters1913.com/words/Elench)
