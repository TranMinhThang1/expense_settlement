# expense_settlement

## Project Title
expense_settlement

## Project Description
`expense_settlement` is a Soroban smart contract that turns a small business's
expense-report process into a tamper-evident, on-chain ledger. Employees file
claims with an amount, a category (e.g. `TRAVEL`, `MEALS`) and the hash of
their receipt; a designated **manager** approves or rejects each claim with an
on-chain reason; an approved claim is then queued for a **finance officer** to
mark as `SETTLED` once the actual disbursement has been paid out. Unlike a P2P
shared-bill splitter, every actor here has a fixed organisational role and the
workflow is strictly *employee → manager → finance*.

## Project Vision
Reimbursement today is buried in email threads, spreadsheets and PDF receipts
that no one can audit later without trusting the accountant. Our vision is to
make the *workflow itself* — who claimed, who approved, who paid — a public,
append-only artefact that any auditor, regulator or new CFO can verify in
seconds, while keeping the sensitive receipt content off-chain (only its hash
is anchored). Long-term, `expense_settlement` aims to become the
"GitHub-for-reimbursements" of the Stellar ecosystem: cheap enough for a
freelancer collective, structured enough for a fast-growing startup, and
auditable enough for a regulated SME.

## Key Features
- **Role-gated workflow** — three distinct on-chain roles (`admin`, `manager`,
  `finance`) enforced with `require_auth()` plus an explicit equality check
  against the address stored at init time.
- **Receipt anchoring** — each claim stores a `BytesN<32>` hash of the
  off-chain receipt (SHA-256 or IPFS CID digest), so the artefact stays
  private but its existence and integrity are publicly provable.
- **Deterministic state machine** — claims transition only along the legal
  path `PENDING → APPROVED → SETTLED` or `PENDING → REJECTED`; any attempt to
  skip or rewind a step panics.
- **Timestamped audit trail** — `submitted_at` and `updated_at` are written
  from `env.ledger().timestamp()` on every transition, giving every claim a
  verifiable lifecycle history.
- **Pluggable role rotation** — the admin can rotate the `MANAGER` or
  `FINANCE` address via `set_role`, so employee turnover does not require
  redeploying the contract.
- **Cheap read API** — `claim_count`, `claim_status`, `get_claim`,
  `get_manager` and `get_finance` let any front-end render the company-wide
  expense dashboard without re-indexing.

## Contract

- **Network:** Stellar Testnet (Public)
- **Scope:** finance dApp — see `contracts/expense_settlement/src/lib.rs` for the full expense_settlement business logic.
- **Functions exposed:** see `Key Features` above and the `pub fn` list in `lib.rs`.
- **Contract ID:** `CCSSIMOTNZKRWK73ZMJGMFYIIOWAVJOC7LIN3GLERIYSOAUBA4IOVRU3`
- **Explorer template:** `https://stellar.expert/explorer/testnet/tx/3b3d35aebc06df9bd7e54c90d6b256a24375b5468a4fb06e9627580e1a849260`

## Future Scope
- **Per-employee spend caps** — reject a `submit_claim` whose rolling 30-day
  total would exceed a manager-configured limit, removing one full review
  round-trip for routine expenses.
- **Multi-approver policies** — require N-of-M manager signatures for claims
  above a configurable threshold (e.g. >$5,000 needs CFO + CEO).
- **Native token settlement** — replace the off-chain payout with an
  in-contract `token::Client` transfer (XLM or company-issued USDC), turning
  `mark_settled` into the actual disbursement.
- **Category budgets & analytics** — track aggregate spend per `Symbol`
  category so finance dashboards can be rendered straight from on-chain
  state.
- **Receipt-revocation flow** — let an auditor flag a settled claim as
  "disputed" so the audit trail records the finding without rewriting
  history.
- **Frontend dApp** — a Freighter-connected React UI for employees to drop a
  receipt PDF (hashed client-side), for managers to approve from their phone,
  and for finance to bulk-settle the day's queue.

## Profile

- **Name:** <!-- Fill github name -->
- **Project:** `expense_settlement` (finance)
- **Built with:** Soroban SDK 25, Rust, Stellar Testnet
