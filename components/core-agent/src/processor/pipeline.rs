// This where the whole data processing pipeline is orchestrated, providing a single
// method that can be called to start the pipeline. This is similar to configuration
// bootstrapping.
//
// Currently, configuration schema for the processor is undefined, this means, the
// processor will determine its own configuration schema. Each part of data processing
// is what builds this file's logic. This file just orchestrates how a pipeline is
// created from its individual logical components