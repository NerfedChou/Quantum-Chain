//! # Multi-Node Consensus
//!
//! Query multiple full nodes and require agreement.
//!
//! Reference: System.md Line 644, SPEC-13 Lines 579-617

use crate::domain::{LightClientError, CONSENSUS_THRESHOLD};

/// Check if responses from multiple nodes reach consensus.
///
/// Reference: SPEC-13 Line 606 - "2/3 agreement"
///
/// # Arguments
/// * `responses` - Responses from full nodes
/// * `min_nodes` - Minimum required nodes
///
/// # Returns
/// * `Ok(T)` - The consensus value
/// * `Err` - Not enough nodes or no consensus
pub fn check_consensus<T: Clone + PartialEq>(
    responses: &[T],
    min_nodes: usize,
) -> Result<T, LightClientError> {
    if responses.len() < min_nodes {
        return Err(LightClientError::InsufficientNodes {
            got: responses.len(),
            required: min_nodes,
        });
    }

    if responses.is_empty() {
        return Err(LightClientError::ConsensusFailed(
            "No responses".to_string(),
        ));
    }

    // Count occurrences of each response
    let mut counts: Vec<(T, usize)> = Vec::new();
    for response in responses {
        let found = counts.iter_mut().find(|(r, _)| r == response);
        match found {
            Some((_, count)) => *count += 1,
            None => counts.push((response.clone(), 1)),
        }
    }

    // Find the most common response
    let (best, count) = counts.into_iter().max_by_key(|(_, c)| *c).unwrap();

    // Check if it meets the threshold
    let threshold = (responses.len() as f64 * CONSENSUS_THRESHOLD).ceil() as usize;
    if count < threshold {
        return Err(LightClientError::ConsensusFailed(format!(
            "Only {}/{} nodes agree (need {})",
            count,
            responses.len(),
            threshold
        )));
    }

    Ok(best)
}

/// Verify all responses are identical (strict consensus).
pub fn check_strict_consensus<T: Clone + PartialEq>(
    responses: &[T],
    min_nodes: usize,
) -> Result<T, LightClientError> {
    if responses.len() < min_nodes {
        return Err(LightClientError::InsufficientNodes {
            got: responses.len(),
            required: min_nodes,
        });
    }

    if responses.is_empty() {
        return Err(LightClientError::ConsensusFailed(
            "No responses".to_string(),
        ));
    }

    let first = &responses[0];
    if responses.iter().all(|r| r == first) {
        Ok(first.clone())
    } else {
        Err(LightClientError::ForkDetected)
    }
}

/// Calculate required number of agreeing nodes for consensus.
pub fn required_for_consensus(total: usize) -> usize {
    (total as f64 * CONSENSUS_THRESHOLD).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_consensus_all_agree() {
        let responses = vec![42, 42, 42];
        let result = check_consensus(&responses, 3);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_check_consensus_2_of_3() {
        let responses = vec![1, 1, 2]; // 2/3 agree on 1
        let result = check_consensus(&responses, 3);
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_check_consensus_supermajority() {
        let responses = vec![1, 1, 1, 2, 3]; // 3/5 agree on 1 (60% >= 2/3?)
                                             // 2/3 of 5 = 3.33 -> 4 required, so this fails
        let result = check_consensus(&responses, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_consensus_insufficient_nodes() {
        let responses = vec![42, 42];
        let result = check_consensus(&responses, 3);
        assert!(matches!(
            result,
            Err(LightClientError::InsufficientNodes { .. })
        ));
    }

    #[test]
    fn test_check_consensus_empty() {
        let responses: Vec<i32> = vec![];
        let result = check_consensus(&responses, 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_strict_consensus_pass() {
        let responses = vec![42, 42, 42];
        let result = check_strict_consensus(&responses, 3);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_check_strict_consensus_fail() {
        let responses = vec![1, 1, 2];
        let result = check_strict_consensus(&responses, 3);
        assert!(matches!(result, Err(LightClientError::ForkDetected)));
    }

    #[test]
    fn test_required_for_consensus() {
        assert_eq!(required_for_consensus(3), 2); // 2/3 of 3 = 2
        assert_eq!(required_for_consensus(5), 4); // 2/3 of 5 = 3.33 -> 4
        assert_eq!(required_for_consensus(9), 6); // 2/3 of 9 = 6
    }
}
