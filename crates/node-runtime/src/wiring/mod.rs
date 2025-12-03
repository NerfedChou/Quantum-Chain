//! # Subsystem Wiring Module
//!
//! This module contains the wiring logic that connects all core subsystems
//! according to the V2.3 Choreography pattern.
//!
//! ## Architecture Principle
//!
//! Each subsystem defines its **ports** (traits). The node-runtime provides
//! **adapters** that implement these ports and wire subsystems together.
//!
//! ## V2.3 Choreography (from Architecture.md)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                     V2.3 CORE SUBSYSTEM WIRING                              │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                             │
//! │  ┌──────────────────────────────────────────────────────────────────────┐   │
//! │  │                         EVENT BUS                                     │   │
//! │  │  (shared-bus crate - authenticated message transport)                 │   │
//! │  └───────────────────────────────┬──────────────────────────────────────┘   │
//! │                                  │                                          │
//! │     ┌─────────────────────┬──────┴──────┬─────────────────────┐             │
//! │     │                     │             │                     │             │
//! │     ▼                     ▼             ▼                     ▼             │
//! │  ┌──────┐             ┌──────┐      ┌──────┐              ┌──────┐          │
//! │  │ 10   │             │  8   │      │  9   │              │  5   │          │
//! │  │ Sig  │◄────────────│Consen│──────│Final │              │Block │          │
//! │  │Verify│             │ sus  │      │ ity  │              │Propag│          │
//! │  └──────┘             └──┬───┘      └──────┘              └──────┘          │
//! │     ▲                    │                                    ▲             │
//! │     │                    │ BlockValidated                     │             │
//! │     │              ┌─────┴─────────────────────────┐          │             │
//! │     │              │                               │          │             │
//! │     │              ▼                               ▼          │             │
//! │  ┌──────┐      ┌──────┐                        ┌──────┐      │             │
//! │  │  6   │      │  3   │                        │  4   │      │             │
//! │  │Mem   │      │ Tx   │                        │State │      │             │
//! │  │pool  │      │Index │                        │ Mgmt │      │             │
//! │  └──────┘      └──┬───┘                        └──┬───┘      │             │
//! │     ▲             │ MerkleRootComputed            │          │             │
//! │     │             │                               │          │             │
//! │     │             └───────────────┬───────────────┘          │             │
//! │     │                             │ StateRootComputed        │             │
//! │     │                             ▼                          │             │
//! │  ┌──────┐                    ┌──────────┐                    │             │
//! │  │  1   │                    │    2     │                    │             │
//! │  │Peer  │◄──────────────────│  Block   │────────────────────┘             │
//! │  │Disc  │                    │ Storage  │                                  │
//! │  └──────┘                    │(Assembler)│                                  │
//! │                              └──────────┘                                  │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Wiring Responsibilities
//!
//! 1. **Port Implementation**: Create adapters that implement subsystem ports
//! 2. **Event Routing**: Connect subsystems via event bus subscriptions
//! 3. **Security**: Ensure all messages use `AuthenticatedMessage<T>` envelope
//! 4. **IPC Authorization**: Verify sender_id per IPC-MATRIX.md rules

pub mod core_subsystems;
pub mod event_routing;

pub use core_subsystems::*;
pub use event_routing::*;
