# Authentication Infrastructure

Goals:

- Seamless authentication of coordinator/administrator, minimal input from
user (except for authorization / initial key decryption)

Ideas:

- Use SSH keypairs, one per "admin" device
  - Private keys are PGP-encrypted and stored locally, never sent over network
- Distribute public keys (and key updates) to inventory using Git
  - Easy key grants/revocations from any admin (add/delete lines from
  authorized_keys and pull everywhere)
- Enforce key rotation
- SSH agent forwarding, could use admin keys for other Git repos, does not need
to rely on keys on managed machines.

Challenges:

- Onboarding new inventory?
  - Need to set up initial configuration to "bootstrap" onto network. Needs to
  get SSH authorized_keys file from somewhere.

- Onboarding new admins?
  - Admin private key should ideally be generated locally.
  - New admin does not have rights to push its keys everywhere.
    - Maybe copy/paste public key to an existing admin is easy enough?
  - Is it desired to automate public key deployment to GitHub etc?

- How to set up ssh-agent within gander?
  - Need an ephemeral ssh-agent that is just for this process & forwarding
  - `russh_keys::agent::server::serve` seems to be exactly this
