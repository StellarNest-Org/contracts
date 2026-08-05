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

