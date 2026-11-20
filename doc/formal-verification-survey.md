# Formal Verification in Nanvix — Survey

Snapshot of upstream `nanvix/nanvix` `dev` at
[`c4c464703`](https://github.com/nanvix/nanvix/commit/c4c464703), surveyed 2026-11-19.
Sources: the tree at that commit, GitHub PRs, issues and Actions runs. Counts below are from
`git grep` on that commit or from the cited CI logs; nothing is estimated.

## TL;DR

- **One tool: [Verus](https://github.com/verus-lang/verus)** (SMT-based Rust verifier, Z3 backend).
  No Kani, Prusti, Creusot, Flux, Coq, Lean, Isabelle, Dafny or TLA+ anywhere in the tree.
- **What is verified:** memory-management building blocks: `bitmap`, `raw-array`, `nanvix-slab`,
  address and frame types in `sys`/`arch`, and in the kernel the **kheap allocator** and the
  **physical frame allocator** (with refcounts).
- **CI-enforced:** a `Verify (microvm)` job (Linux) and `Windows Verify (microvm)` run on every
  non-draft PR, merge-group and nightly. Both passed on the 2026-11-19 nightly
  ([run 44474839849](https://github.com/nanvix/nanvix/actions/runs/44474839849)).
- **Who:** `tangruize` (bitmap, raw-array, kheap, phys-mem specs and proofs), `jaylorch`
  (Slab, attribute-style Verus, open virtual-memory specs PR), `ppenna` (maintainer, CI, tooling,
  docs, toolchain bumps).
- **Live problem:** the weekly Verus auto-update has **failed 7 weeks in a row (2026-10-05 →
  2026-11-16)**. Each new Verus release breaks one assertion in the Slab proof, so the pin is
  stuck at `0.2026.09.27.3cf1832` (see [Gaps](#gaps-and-open-problems)).
- **In flight:** draft [nanvix/nanvix#3136](https://github.com/nanvix/nanvix/pull/3136)
  "Add virtual memory specs" (jaylorch, +3670/−180, 37 files, last updated 2026-09-11).

## 1. Tooling and wiring

| Piece | Location | Notes |
|---|---|---|
| Pinned Verus release | `build/verus-version` | `0.2026.09.27.3cf1832` |
| Installer | `scripts/setup/verus.py` | Cross-platform download + cache; `./z setup --verus` installs to `~/verus` |
| Make targets | `Makefile` (`verify`, `verify-<crate>`, `ensure-verus`) | `VERUS_CRATES := bitmap sys nanvix-slab arch kernel`; kernel verified with features `microvm trace` |
| Verify command | `Makefile` `VERUS_VERIFY_CMD` | `RUSTC_BOOTSTRAP=1 cargo verus verify --locked --no-default-features` |
| Opt-in per crate | `[package.metadata.verus] verify = true` | in `bitmap`, `sys`, `slab`, `arch`, `kernel`, `raw-array`, `error`, `type-safe`, `hostfs-api` |
| CI job | `.github/workflows/ci.yml` (`verify`, Windows verify), `.github/actions/verify/action.yml` | 60 min timeout, `microvm` only, caches the Verus archive |
| Auto-update | `.github/workflows/verus-update.yml` + `scripts/update-verus.sh` | Mondays 06:00 UTC; bumps Verus + `vstd`, re-verifies, opens a PR |
| Tests for tooling | `tests/test_verus.py`, `tests/test_update_verus.py` | |
| Docs | `doc/verus-for-kernel-maintainers.md` (659 lines), `doc/build-{linux,windows}.md`, `doc/setup-{linux,windows}.md`, `CONTRIBUTING.md` | The maintainer guide teaches how to *read* contracts without writing proofs |

`raw-array`, `error`, `type-safe` and `hostfs-api` are not in `VERUS_CRATES`, so they are only
verified when a listed crate pulls them in as a dependency.

### Code conventions

- Contracts go on mainline functions via `#[verus_spec(...)]` (attribute mode, adopted in
  [nanvix/nanvix#1599](https://github.com/nanvix/nanvix/pull/1599)); some code still uses `verus! { }` blocks.
- Each verified module splits into `x.rs` (code + contracts), `x.spec.rs` (views, `inv()`,
  spec fns, `assume_specification`s) and `x.proof.rs` (lemmas). `bitmap` also has `lib.test.rs`.
- Specs and proofs are erased in normal builds (`cfg(verus_keep_ghost)`).

## 2. What is verified

Per-crate footprint at `c4c464703`. *Spec/proof lines* = total lines in `*.spec.rs`, `*.proof.rs`,
`*.test.rs`. *Trusted* = `external_body` / `assume_specification` / `#[verifier::external*]`
occurrences. No `admit()` or `assume(` calls were found in any of these crates.

| Crate | Path | Spec/proof lines | `#[verus_spec]` | Trusted | Verified in CI (2026-11-19) |
|---|---|---:|---:|---:|---|
| bitmap | `src/libs/bitmap` | 1510 | 15 | 0 | 68 verified, 0 errors |
| nanvix-slab | `src/libs/slab` | 916 | 5 | 7 | 38 verified, 0 errors |
| sys (addresses, alignment) | `src/libs/sys/src/sys/mm` | 60 | 4 | 10 | 17 verified, 0 errors |
| arch (x86 frame numbers) | `src/libs/arch/src/x86/mem/paging/frame` | 137 | 6 | 2 | 17 verified, 0 errors |
| raw-array | `src/libs/raw-array` | 80 | 4 | 5 | 0 verified (contracts only, checked as a dependency) |
| kernel (kheap, phys frames, HAL address/region types) | `src/kernel/src/{mm,hal}` | 3594 | 26 | 5 | 137 verified, 0 errors |

Across the tree there are 131 `proof fn`s. CI counts come from the
[Verify (microvm) job log](https://github.com/nanvix/nanvix/actions/runs/44474839849), matched
to the `Checking <crate>` line printed before each result. The same log also prints a
`1897 verified` line in the bitmap run that is not attributed to a crate (it appears to be `vstd`
itself); it is left out of the table.

### Highlights

- **Kernel heap (`src/kernel/src/mm/kheap*.rs`).** The caller-visible view is
  `KheapView { allocations: Map<addr, size> }`. `inv()` says every allocation has a positive size,
  a non-null address, and no two allocations overlap. Allocation and deallocation are specified
  against `spec_allocate` / `spec_deallocate`. `kheap.proof.rs` is about 1840 lines. Landed in
  [#2539](https://github.com/nanvix/nanvix/pull/2539) (2026-06-15), after prep PRs
  [#2373](https://github.com/nanvix/nanvix/pull/2373) and [#2505](https://github.com/nanvix/nanvix/pull/2505).
- **Physical frame allocator (`src/kernel/src/mm/phys/`).** The view is a single map from frame
  address to reference count (0 = free, >0 = allocated, >1 = shared/COW). Specs landed in
  [#2815](https://github.com/nanvix/nanvix/pull/2815), proofs in [#2971](https://github.com/nanvix/nanvix/pull/2971) (2026-07).
- **Slab (`src/libs/slab`).** Contracts and proofs for the slab allocator ("Verify Slab"), plus a
  "Prove liveness of Slab constructor" commit ([#1899](https://github.com/nanvix/nanvix/pull/1899), `de06273ab`, `a86f39b78`) and a
  `free_addrs` postcondition ([#2382](https://github.com/nanvix/nanvix/pull/2382)).
- **Bitmap / raw-array.** These were the first verified crates
  ([#1378](https://github.com/nanvix/nanvix/pull/1378) enabled Verus, 2026-02-18;
  [#1467](https://github.com/nanvix/nanvix/pull/1467) added proofs, 2026-03-04).
- **Verification already caught real bugs.** Proof work led to fixes such as
  [#1479](https://github.com/nanvix/nanvix/pull/1479) (bitmap `alloc_range` wrap-around),
  [#1798](https://github.com/nanvix/nanvix/pull/1798) (slab overflow),
  [#2190](https://github.com/nanvix/nanvix/pull/2190) (kpool `usize` underflow),
  [#2554](https://github.com/nanvix/nanvix/pull/2554) and [#2555](https://github.com/nanvix/nanvix/pull/2555) (kernel address-math panics).

### Trusted base (what is assumed, not proved)

- `assume_specification`s for raw-pointer ops in `slab/lib.spec.rs` (`is_null`, `add`,
  `wrapping_add`, pointer comparisons, `offset_from_unsigned`) and for `core::alloc::Layout::{size, align}`
  in `kheap.spec.rs`. `Layout` itself is `#[verifier::external_body]`.
- Verus, Z3, `vstd` and rustc at the pinned versions.
- Everything outside `VERUS_CRATES`: the rest of the kernel (scheduler, IPC, VFS, virtual memory,
  syscalls) and all of userspace/libc carry no proofs.

## 3. Timeline

| Date | Event |
|---|---|
| 2026-02-18 | Verus enabled on `bitmap` ([#1378](https://github.com/nanvix/nanvix/pull/1378), jaylorch) |
| 2026-02-27 → 03-04 | Setup automation and CI caching ([#1475](https://github.com/nanvix/nanvix/pull/1475), [#1522](https://github.com/nanvix/nanvix/pull/1522), [#1541](https://github.com/nanvix/nanvix/pull/1541)) |
| 2026-03-04 | bitmap + raw-array proofs ([#1467](https://github.com/nanvix/nanvix/pull/1467), tangruize) |
| 2026-03-12 | Attribute-style contracts ([#1599](https://github.com/nanvix/nanvix/pull/1599)); weekly Verus update workflow ([#1614](https://github.com/nanvix/nanvix/pull/1614)) |
| 2026-04-01 | Slab verified ([#1899](https://github.com/nanvix/nanvix/pull/1899), jaylorch) |
| 2026-04-02 | Windows verification ([#1929](https://github.com/nanvix/nanvix/pull/1929)); maintainer guide ([#1922](https://github.com/nanvix/nanvix/pull/1922)) |
| 2026-06-15 | kheap allocator verified ([#2539](https://github.com/nanvix/nanvix/pull/2539)) |
| 2026-06-29 | Proofs adapted to new Verus `&mut` semantics ([#2777](https://github.com/nanvix/nanvix/pull/2777), `50088648c`) |
| 2026-07-21 / 07-28 | Physical memory specs / proofs ([#2815](https://github.com/nanvix/nanvix/pull/2815), [#2971](https://github.com/nanvix/nanvix/pull/2971)) |
| 2026-08-25 | Virtual memory specs opened as a draft ([#3136](https://github.com/nanvix/nanvix/pull/3136), jaylorch) |
| 2026-09-28 | Last successful Verus bump (`f248df6e3`, → `0.2026.09.27.3cf1832`) |
| 2026-10-05 → 11-16 | Verus auto-update fails every week (Slab proof) |
| 2026-11-14 | Verus gates updated for Rust 1.100 ([#3304](https://github.com/nanvix/nanvix/pull/3304), issue [#3299](https://github.com/nanvix/nanvix/issues/3299)) |

## 4. Gaps and open problems

1. **The Verus pin is stuck because of one Slab assertion.** Every `verus-update` run since
   2026-10-05 (Verus `0.2026.10.04`, `.10.18`, `.10.26`, `.11.02`) re-verifies and fails in
   `nanvix-slab` with `37 verified, 1 errors`:
   `src/libs/slab/src/lib.proof.rs:398: assert(diff == j * bs);` (just after
   `lemma_fundamental_div_mod(diff, bs)`). This is likely an arithmetic or trigger change in
   newer `vstd`/Verus. The fix is probably a one-line proof tweak, e.g. using the lemma's
   `diff == bs * (diff / bs) + diff % bs` form explicitly or a `by (nonlinear_arith)`. Not yet
   tried upstream. A candidate fix (Verus bump plus a `by (nonlinear_arith)` step) is on
   [`maceip/nanvix:fix-slab-proof-verus-update`](https://github.com/maceip/nanvix/tree/fix-slab-proof-verus-update);
   it verifies all five crates locally on Verus 0.2026.11.02 (macOS arm64) with 0 errors. Latest failing run:
   [verus-update on 2026-11-16](https://github.com/nanvix/nanvix/actions/workflows/verus-update.yml).
2. **Virtual memory is unverified.** [#3136](https://github.com/nanvix/nanvix/pull/3136) is a
   draft marked "do not merge" and has been idle since 2026-09-11.
3. **Coverage is limited to memory allocators and address types.** Scheduler, IPC, VFS,
   syscall dispatch and page-table walks have no contracts.
4. **Only `microvm` is verified.** The CI matrix is `machine-type: [ microvm ]`, kernel features
   `microvm trace`, target `x86`. x86_64 and other machine types are not run through Verus.
5. **The trusted pointer specs in `slab` are hand-written** `assume_specification`s. Any mismatch
   with real `core` semantics would be unsound and would go undetected.

## 5. Run it locally

```bash
./z setup --verus            # installs the pinned Verus to ~/verus
./z build -- verify          # verify all VERUS_CRATES
./z build -- verify-nanvix-slab   # one crate (bitmap | sys | nanvix-slab | arch | kernel)
# custom install:  ./z build -- verify VERUS_EXECUTABLE_DIR=/path/to/verus
# skip:            VERUS_EXECUTABLE_DIR= ./z build -- verify
```

To reproduce gap #1, set `build/verus-version` to `0.2026.11.02.8bf68ee`, bump `vstd` to
`0.0.0-2026-11-01-0251` (which is what `scripts/update-verus.sh` does), then run
`./z build -- verify-nanvix-slab`.
