searchState.loadedDescShard("iroh_metrics", 0, "Metrics library for iroh\nPotential errors from this library.\nAny IO related error.\nIndicates that the metrics have not been enabled.\nConfiguration for pushing metrics to a remote endpoint.\nExposes core types and traits\nDecrements the given gauge by 1.\nDecrements the given gauge <code>n</code>.\nThe endpoint url for the push metrics collector.\nReturns the argument unchanged.\nReturns the argument unchanged.\nIncrements the given counter or gauge by 1.\nIncrements the given counter or gauge by <code>n</code>.\nThe name of the instance you’re exporting metrics for.\nThe push interval in seconds.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nMetrics collection\nParses Prometheus metrics from a string.\nThe password for basic auth for the push metrics collector.\nThe name of the service you’re exporting metrics for.\nSets the given counter or gauge to <code>n</code>.\nReexports <code>struct_iterable</code> to make matching versions easier.\nThe username for basic auth for the push metrics collector.\nCore is the base metrics struct.\nOpen Metrics <code>Counter</code> to measure discrete events.\nOpen Metrics <code>Gauge</code>.\nInterface for all distribution based metrics.\nDescription of a group of metrics.\nReturns the metric item representation.\nInterface for all single value based metrics.\nThe actual prometheus counter.\nDecrease the <code>Gauge</code> by 1, returning the previous value.\nDecrease the <code>Gauge</code> by <code>i64</code>, returning the previous value.\nReturns the metrics descriptions.\nWhat this counter measures.\nWhat this gauge tracks.\nThe description of the metric.\nEncodes the current metrics registry to a string in the …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nThe actual prometheus gauge.\nReturns a reference to the core metrics.\nGet the current value of the <code>Counter</code>.\nGet the <code>Gauge</code> value.\nReturns a reference to the mapped metrics instance.\nIncrease the <code>Counter</code> by 1, returning the previous value.\nIncrease the <code>Gauge</code> by 1, returning the previous value.\nIncrease the <code>Counter</code> by <code>u64</code>, returning the previous value.\nIncrease the <code>Gauge</code> by <code>i64</code>, returning the previous value.\nMust only be called once to init metrics.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThe name of this metric group.\nReturns the name of the metric\nReturns the name of the metric\nThe name of the metric.\nInitializes this metric group.\nConstructs a new counter, based on the given <code>description</code>.\nConstructs a new gauge, based on the given <code>description</code>.\nReturns a reference to the prometheus registry.\nSet the <code>Counter</code> value. Warning: this is not default …\nSet the <code>Gauge</code> value.\nAttempts to get the current metric from the global …\nTrieds to init the metrics.\nThe type of the metric.\nAccess to this metrics group to record a metric. Only …\nStart a metrics dumper service.\nStart a metrics exporter service.\nStart a server to serve the OpenMetrics endpoint.")