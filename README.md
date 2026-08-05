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

