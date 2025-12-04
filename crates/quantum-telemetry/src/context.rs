//! Trace context propagation for the event bus.
//!
//! When a message crosses subsystem boundaries via the event bus,
//! trace context must be propagated to maintain distributed tracing.
//!
//! ## How It Works
//!
//! 1. Subsystem A creates a span and extracts context as `PropagatedContext`
//! 2. Context is serialized into the event message
//! 3. Subsystem B receives the message and injects the context
//! 4. Subsystem B's spans become children of Subsystem A's span
//!
//! ## Example
//!
//! ```rust,ignore
//! // In Consensus (Subsystem 8) - publishing BlockValidated
//! let span = tracing::info_span!("validate_block", block_height = 12345);
//! let _guard = span.enter();
//!
//! // Extract context for propagation
//! let context = TraceContext::extract_current();
//!
//! // Include in event message
//! let event = BlockValidatedEvent {
//!     block_hash: hash,
//!     trace_context: context.to_propagated(),
//! };
//!
//! // In Transaction Indexing (Subsystem 3) - receiving BlockValidated
//! let parent_context = event.trace_context.to_context();
//! let span = parent_context.child_span("compute_merkle_root");
//! let _guard = span.enter();
//! ```

use opentelemetry::{
    trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState},
    Context,
};
use serde::{Deserialize, Serialize};

/// Trace context that can be serialized and sent across process boundaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagatedContext {
    /// Trace ID (32 hex characters)
    pub trace_id: String,
    /// Parent span ID (16 hex characters)
    pub span_id: String,
    /// Trace flags (sampled, etc.)
    pub trace_flags: u8,
    /// Optional trace state (vendor-specific data)
    pub trace_state: Option<String>,
}

impl PropagatedContext {
    /// Create an empty context (no parent trace)
    pub fn empty() -> Self {
        Self {
            trace_id: "00000000000000000000000000000000".to_string(),
            span_id: "0000000000000000".to_string(),
            trace_flags: 0,
            trace_state: None,
        }
    }

    /// Check if this context is valid (has a real trace)
    pub fn is_valid(&self) -> bool {
        !self.trace_id.chars().all(|c| c == '0')
    }

    /// Convert to OpenTelemetry Context for creating child spans.
    pub fn to_context(&self) -> TraceContext {
        if !self.is_valid() {
            return TraceContext::new();
        }

        // Parse trace ID
        let trace_id = TraceId::from_hex(&self.trace_id).unwrap_or(TraceId::INVALID);

        // Parse span ID
        let span_id = SpanId::from_hex(&self.span_id).unwrap_or(SpanId::INVALID);

        // Parse trace flags
        let trace_flags = TraceFlags::new(self.trace_flags);

        // Parse trace state
        let trace_state = self
            .trace_state
            .as_ref()
            .and_then(|s| TraceState::from_key_value(vec![("qc", s.as_str())]).ok())
            .unwrap_or_default();

        // Build span context
        let span_context = SpanContext::new(
            trace_id,
            span_id,
            trace_flags,
            true, // remote = true (came from another process)
            trace_state,
        );

        TraceContext {
            span_context: Some(span_context),
        }
    }
}

/// Wrapper for OpenTelemetry Context with helper methods.
pub struct TraceContext {
    span_context: Option<SpanContext>,
}

impl TraceContext {
    /// Create a new empty trace context.
    pub fn new() -> Self {
        Self { span_context: None }
    }

    /// Extract the current trace context from the active span.
    pub fn extract_current() -> Self {
        let context = Context::current();
        let span_context = context.span().span_context().clone();

        Self {
            span_context: if span_context.is_valid() {
                Some(span_context)
            } else {
                None
            },
        }
    }

    /// Convert to a propagatable format for serialization.
    pub fn to_propagated(&self) -> PropagatedContext {
        match &self.span_context {
            Some(ctx) if ctx.is_valid() => PropagatedContext {
                trace_id: ctx.trace_id().to_string(),
                span_id: ctx.span_id().to_string(),
                trace_flags: ctx.trace_flags().to_u8(),
                trace_state: None, // Simplified for now
            },
            _ => PropagatedContext::empty(),
        }
    }

    /// Create a child span linked to this context.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let parent = event.trace_context.to_context();
    /// let span = parent.child_span_for_subsystem("consensus", "process_block");
    /// let _guard = span.enter();
    /// ```
    pub fn child_span_for_subsystem(&self, subsystem: &str, operation: &str) -> tracing::Span {
        match &self.span_context {
            Some(ctx) if ctx.is_valid() => {
                tracing::info_span!(
                    "subsystem_operation",
                    otel.trace_id = %ctx.trace_id(),
                    otel.parent_id = %ctx.span_id(),
                    subsystem = %subsystem,
                    operation = %operation,
                )
            }
            _ => tracing::info_span!(
                "subsystem_operation",
                subsystem = %subsystem,
                operation = %operation,
            ),
        }
    }

    /// Check if this context has a valid trace.
    pub fn is_valid(&self) -> bool {
        self.span_context
            .as_ref()
            .map(|c| c.is_valid())
            .unwrap_or(false)
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper trait for adding trace context to event bus messages.
pub trait WithTraceContext {
    /// Get the propagated trace context from this message.
    fn trace_context(&self) -> &PropagatedContext;

    /// Set the propagated trace context on this message.
    fn set_trace_context(&mut self, context: PropagatedContext);
}

/// Macro to add trace context field to an event struct.
#[macro_export]
macro_rules! with_trace_context {
    ($struct_name:ident { $($field:ident: $type:ty),* $(,)? }) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $struct_name {
            $(pub $field: $type,)*
            pub trace_context: $crate::PropagatedContext,
        }

        impl $crate::WithTraceContext for $struct_name {
            fn trace_context(&self) -> &$crate::PropagatedContext {
                &self.trace_context
            }

            fn set_trace_context(&mut self, context: $crate::PropagatedContext) {
                self.trace_context = context;
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_context() {
        let ctx = PropagatedContext::empty();
        assert!(!ctx.is_valid());
    }

    #[test]
    fn test_context_roundtrip() {
        let propagated = PropagatedContext {
            trace_id: "0af7651916cd43dd8448eb211c80319c".to_string(),
            span_id: "b7ad6b7169203331".to_string(),
            trace_flags: 1,
            trace_state: None,
        };

        assert!(propagated.is_valid());

        let context = propagated.to_context();
        let back = context.to_propagated();

        assert_eq!(propagated.trace_id, back.trace_id);
        assert_eq!(propagated.span_id, back.span_id);
    }

    #[test]
    fn test_extract_current_empty() {
        // No active span, should return empty context
        let ctx = TraceContext::extract_current();
        assert!(!ctx.is_valid());
    }
}
