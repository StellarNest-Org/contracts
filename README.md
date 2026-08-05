# <img src="assets/logo.svg" width="32" height="32" align="center" alt="" /> StellarNest — Contracts

Soroban smart contracts powering **StellarNest**, a family financial
coordination platform on Stellar. Instead of a joint bank account or a
shared spreadsheet, a family gets one programmable treasury: shared
savings, bill automation, spend approvals, and inheritance — all enforced
on-chain, not by convention or a support team's goodwill.

> One treasury. One family. Infinite trust.

This repo contains a single contract, `treasury`, plus the tooling to
build, test, and deploy it. It's one of three StellarNest repos:

| Repo | Purpose |
|---|---|
| [`contracts`](https://github.com/StellarNest-Org/contracts) *(this repo)* | The Soroban `treasury` contract |
| [`backend`](https://github.com/StellarNest-Org/backend) | GraphQL API, Postgres data layer, non-custodial Stellar integration |
| [`frontend`](https://github.com/StellarNest-Org/frontend) | Marketing site + product preview (Next.js) |

## Table of contents

- [New to Stellar/Soroban? Start here](#new-to-stellarsoroban-start-here)
- [Why a smart contract, not just a database](#why-a-smart-contract-not-just-a-database)
- [Core model](#core-model)
- [Method reference](#method-reference)
- [Errors](#errors)
- [Storage layout](#storage-layout)
- [A walkthrough: the Adeyemi family treasury](#a-walkthrough-the-adeyemi-family-treasury)
- [Development](#development)
- [Testing](#testing)
- [Deployment](#deployment)
- [Design principles](#design-principles)
- [Contributing](#contributing)

## New to Stellar/Soroban? Start here

You don't need blockchain experience to read this repo, but a few terms
come up constantly. Here's what they mean, in plain language:

- **Stellar** is a public payments network/blockchain — like a shared,
  global ledger that anyone can read and, with the right permission,
  write to. It's optimized for moving money: transactions confirm in
  3–5 seconds and cost a fraction of a cent, which is why it's a
  reasonable foundation for something as routine as a weekly allowance.
- **Soroban** is Stellar's smart contract platform. A "smart contract"
  is just a program that lives on the ledger itself — once deployed,
  its code runs exactly as written, and nobody (including the people
  who wrote it) can quietly change what it does without deploying a new
  version. That's the whole reason this project uses one: a spending
  rule enforced by a smart contract is a rule that actually holds, not
  a policy someone could bend.
- **A contract instance** is one deployment of this code, identified by
  a unique **contract id** (a long string like `CABC...`). StellarNest
  deploys the `treasury` contract *once* and every family's treasury
  lives inside that same instance, distinguished by a `treasury_id`
  number — see [Core model](#core-model) for why.
- **An account / address** on Stellar is identified by a public key
  (starting with `G...` for a regular account, `C...` for a contract).
  Every person interacting with a treasury — Amara, Chidi, a guardian —
  has their own Stellar address, the same way they'd have their own
  bank account number.
- **Signing / authorization.** Nothing moves on Stellar without a
  signature from the relevant private key. In this contract, that shows
  up as `caller.require_auth()` — a line that says "this call only
  proceeds if `caller` has cryptographically proven they authorized it
  in this exact transaction." Nobody, including StellarNest, can forge
  that.
- **XDR** is Stellar's wire format for transactions — a blob of bytes
  that represents "do this operation." You'll see it mentioned in the
  [backend README](https://github.com/StellarNest-Org/backend): the
  usual flow is *build unsigned XDR → the user's wallet signs it →
  submit the signed XDR to the network*. This contract doesn't deal
  with XDR directly; that happens one layer up.
- **The ledger** is Stellar's version of a "block" — a batch of
  confirmed transactions with a sequence number that increases roughly
  every 5 seconds. This contract uses ledger sequence numbers (not wall
  clock time) to track when a bill is due or when a dead-man switch
  expires, because the ledger's sequence number is the one clock every
  node on the network agrees on.
- **A Stellar Asset Contract (SAC)** is how a currency — XLM, USDC,
  EURC, a custom token — shows up to a Soroban contract: as another
  contract with a standard `transfer`/`balance` interface
  (`token::Client` in this codebase). This contract never invents its
  own accounting for money; it always calls into the real asset's
  contract to move it.
- **Testnet vs. mainnet.** Testnet is Stellar's free practice network —
  same software, fake money, safe to break things on. Mainnet is the
  real network with real value. `scripts/deploy.sh` can target either.

If you're comfortable with those seven ideas, the rest of this README
should read like a normal backend service's docs — because that's
mostly what a Soroban contract is: a small, extremely strict backend
that happens to run somewhere nobody (not even its author) can quietly
edit it.

## Why a smart contract, not just a database

A rule that lives in application code is a rule someone can quietly change
— an engineer under pressure, a compromised admin panel, a support ticket
that "just this once" bypasses the approval flow. A rule enforced by a
Soroban contract can't be: the contract checks the approval threshold,
the spending limit, and the freeze state *before it will move a single
token*, regardless of what the backend or frontend tell it to do. That
property — rules even StellarNest itself can't override — is the whole
reason this is a contract and not just a Postgres table.

## Core model

`contracts/treasury` is a **single deployed contract instance** that
manages *many* family treasuries, each identified by a `u64` id (the same
multi-tenant-per-contract pattern used by most production Soroban apps,
rather than deploying a fresh contract per family). It intentionally holds
**no custodial keys** — every state-changing call requires the caller's
own `require_auth()`, and every asset movement happens through the
standard Stellar Asset Contract `token` interface (`token::Client`), never
through an internal ledger only the contract admin can move.

| Concept | What it is |
|---|---|
| **Treasury** | One per family. Denominated in a single Stellar asset (XLM, USDC, EURC, AQUA, or any Stellar Asset Contract). Tracks `balance`, `frozen` state, and the approval rule (`approval_threshold`, `required_approvals`). |
| **Members & roles** | `Owner`, `Parent`, `Guardian`, `Child`, `Advisor`, `Viewer`. Roles gate which calls succeed (see [Method reference](#method-reference)). A `Child` member can additionally carry a per-transaction `spending_limit`. |
| **Rules engine** | Withdrawals *below* `approval_threshold` execute immediately, subject to the caller's spending limit. Withdrawals *at or above* it become a pending `Withdrawal` that needs `required_approvals` signatures from Owner/Parent/Guardian members before funds move. |
| **Savings goals** | Named targets (`Emergency Fund`, `Vacation`, ...) with their own `current_amount`, tracked independently of the treasury's free balance so progress is easy to display without a separate ledger. |
| **Bills** | Recurring payments with an on-chain due schedule (`next_due_ledger`). `pay_bill` is **permissionless** but only succeeds once the schedule says it's due — a cron job or relayer can call it without ever being trusted with payment timing or amounts. |
| **Inheritance vault** | Beneficiaries with *basis-point* allocations — basis points are just percentages with more precision (100 bps = 1%, so 10,000 bps = 100%); the contract requires every beneficiary's share to sum to exactly `10_000` so nothing is over- or under-allocated. A `time_lock_ledger` (see [glossary](#new-to-stellarsoroban-start-here) — this is a ledger *sequence number*, not a calendar date) and a dead-man switch the Owner resets via `heartbeat` both gate the claim. Claiming requires guardian approvals **and** either the time-lock or the dead-man switch to have elapsed, then distributes the full treasury balance pro-rata (proportionally, by each beneficiary's percentage) in one transaction. |

Roles map to permissions like this (see `Role::can_administer`,
`Role::can_approve`, `Role::can_spend` in `types.rs`):

| Role | Administer (members/rules/goals/bills) | Approve withdrawals | Can request a spend | Notes |
|---|:---:|:---:|:---:|---|
| Owner | ✅ | ✅ | ✅ | Also the only role that can create/heartbeat the inheritance vault |
| Parent | ✅ | ✅ | ✅ | |
| Guardian | freeze only | ✅ | ✅ | Also the only role that can approve inheritance claims |
| Child | ❌ | ❌ | ✅ (subject to `spending_limit`) | |
| Advisor | ❌ | ❌ | ✅ | |
| Viewer | ❌ | ❌ | ❌ | Read-only |

