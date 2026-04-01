# Multisig Proposal Expiry

## Overview

The contract includes a minimal multisig admin module used for sensitive administrative changes (e.g. freezing the contract, rotating owners, changing threshold, changing proposal duration).

Each multisig proposal has a deterministic `expiry` timestamp stored on-chain. After expiry, the proposal can no longer be approved or executed.

## Storage and Types

- Proposals are stored in persistent storage under `DataKey::MultisigProposal(u32)`.
- Proposal payload is represented by:
  - `ProposalAction` (the requested admin change)
  - `Proposal` (proposal metadata including approvals and expiry)

The `Proposal` struct includes:

- `id: u32`
- `action: ProposalAction`
- `proposer: Address`
- `approvals: Vec<Address>`
- `executed: bool`
- `expiry: u64`

## Expiry Semantics

### How expiry is computed

When creating a proposal via `propose_action`, expiry is calculated as:

- `now = env.ledger().timestamp()`
- `expiry = now + proposal_duration`

The implementation uses checked arithmetic (`checked_add`) and fails if the addition would overflow.

`proposal_duration` is set during `init_multisig` and can later be updated through a multisig proposal (`ProposalAction::SetProposalDuration`).

### When a proposal is considered expired

A proposal is considered expired when:

- `env.ledger().timestamp() >= proposal.expiry`

This boundary is intentional and deterministic. If `now == expiry`, the proposal is already expired.

### What expiry blocks

If a proposal is expired, the following entrypoints return `Err(RevoraError::ProposalExpired)`:

- `approve_action`
- `execute_action`

`get_proposal` remains readable regardless of expiry.

## Security Assumptions and Abuse Paths

- Expiry prevents execution of stale proposals after long inactivity or after off-chain coordination context has changed.
- Expiry does not delete proposal data. Proposals remain in storage for auditability.
- An attacker cannot extend the life of an already-created proposal. Updating `MultisigProposalDuration` only affects proposals created after the update.
- Expiry is based on ledger timestamp. This assumes the ledger timestamp is the authoritative time source for the chain.

## Test Coverage

Deterministic tests validate expiry behavior at the exact boundary (`now == expiry`):

- Approval fails at expiry.
- Execution fails at expiry.

See `src/test.rs`:

- `multisig_approve_fails_after_expiry_boundary`
- `multisig_execute_fails_after_expiry_boundary`
