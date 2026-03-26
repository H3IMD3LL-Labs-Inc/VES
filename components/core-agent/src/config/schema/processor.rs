// =============================================================================
// This configuration schema defines possible configurations for the Core Agent
// SourcePayload processing pipeline. For Core Agent v1.0.0, this will be
// minimal. But later on, will provide parameters that control how events are
// processed before being sent to the HEIMDELL Server as EnrichedEvents
//
// Possible Core Agent Processor configurations:
// - Parser behavior
// - Entity Extraction Settings
// - Fingerprinting Algorithm
// - Relationship tagging controls
// - Pipeline Parallelism
//
// This exists purely to ensure processor pipeline behavior can evolve without
// affecting source drivers configuration or agent runtime configurations
// =============================================================================

#[derive(Default, Clone, PartialEq)]
pub struct ProcessorConfig {}
