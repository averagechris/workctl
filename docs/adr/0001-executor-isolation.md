# ADR 0001: Executor isolation strategy

- Status: accepted
- Date: 2026-07-02
- Deciders: Chris (project owner)

## Context

workctl needs per-task isolated runtimes beyond the first `local-devshell`
process executor. The long-term vision names microVMs and Kubernetes pods.
Two deployment styles must both be first-class with no lock-in
(`docs/requirements.md`):

1. **Personal:** a NixOS server configured by a Nix module. Single node, KVM
   available, no cluster.
2. **Work:** Kubernetes (EKS), Docker images pushed to AWS ECR, deployed with
   helm. Standard EC2 nodes — **no nested virtualization / no `/dev/kvm`**
   without `.metal` instances.

Constraints and observations:

- Dedicated microVM backends (firecracker/cloud-hypervisor, microvm.nix)
  give the strongest isolation but hard-require KVM, which the work
  environment does not offer. Choosing microVMs as the primary model forks
  the architecture between the two deployment styles.
- Kubernetes-only execution makes the personal single-node server carry a
  cluster (k3s) just to run tasks.
- Externally triggered execution (Milestone 4: Linear reactions) raises the
  isolation bar: eventually tasks run without a human watching submission.

## Decision

**The task runtime contract is an OCI container: image + mounts + env +
resource limits. Isolation strength is a runtime configuration knob, not a
separate executor architecture.**

Preference order for `ExecutorBackend` implementations:

1. `container` — runs each task in an OCI container via podman (or docker) on
   the node. First non-local backend; serves the personal NixOS server with
   no cluster dependency.
2. `k8s-job` — runs each task as a Kubernetes Job using the same image
   contract; serves the work deployment (ECR + helm).
3. Isolation hardening rides the same contract: kata-containers /
   firecracker-backed OCI runtimes or gVisor via podman runtime selection on
   NixOS (KVM available), and `runtimeClass` (kata/gVisor) on Kubernetes
   where node support exists. Policy can require a minimum isolation level
   per task class (e.g. externally triggered tasks require VM-grade or
   sandboxed runtimes).

A dedicated microvm.nix executor remains a possible future backend behind the
same `ExecutorBackend` seam, but it is not the preferred path.

## Consequences

- One task image contract serves both deployment styles; no architectural
  fork and no lock-in.
- The `ExecutorBackend` seam is finally proven by a second real
  implementation (`container`), per the "avoid early complexity" rule.
- Task environments shift from ambient host tooling (`nix develop` on a
  cloned repo) toward declared images; the container backend must define how
  repos, secrets, prompts, and artifact paths are mounted into the container.
- Isolation level becomes visible policy/config (`runtime` selection) rather
  than an implementation detail, which Milestone 4 can require for
  externally triggered tasks.
- KVM-dependent isolation is simply unavailable at work until node support
  exists; gVisor-style sandboxing is the ceiling there. Accepted.

## Alternatives considered

- **MicroVMs preferred (microvm.nix):** strongest isolation and an elegant
  NixOS story, but unusable on standard EKS nodes; would force a second,
  divergent execution model for work. Rejected as the primary path.
- **Kubernetes everywhere:** matches work exactly but taxes the personal
  server with a cluster and couples the core to Kubernetes APIs. Rejected as
  the primary path; retained as backend #2.
