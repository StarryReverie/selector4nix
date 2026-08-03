use std::sync::Arc;

use serde::Serialize;

use crate::domain::nar_info::model::NarUrlRewriteOption;
use crate::domain::substituter::model::PeriodicProbingOption;
use crate::infrastructure::config::AppConfiguration;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ConfigSummaryData {
    sections: Vec<ConfigSummarySectionData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ConfigSummarySectionData {
    title: &'static str,
    entries: Vec<ConfigSummaryEntryData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ConfigSummaryEntryData {
    name: &'static str,
    description: &'static str,
    value: String,
}

pub struct GetDashboardConfigSummaryUseCase {
    config: Arc<AppConfiguration>,
}

impl GetDashboardConfigSummaryUseCase {
    pub fn new(config: Arc<AppConfiguration>) -> Self {
        Self { config }
    }

    pub fn run(&self) -> ConfigSummaryData {
        ConfigSummaryData {
            sections: vec![
                self.network_section(),
                self.proxy_section(),
                self.cache_info_section(),
            ],
        }
    }

    fn network_section(&self) -> ConfigSummarySectionData {
        let cfg = &self.config.network;
        ConfigSummarySectionData {
            title: "Network",
            entries: vec![
                ConfigSummaryEntryData {
                    name: "Tolerance",
                    description: "Latency tolerance window in milliseconds per unit of difference of priority between two substituters.",
                    value: format!("{}ms", cfg.tolerance),
                },
                ConfigSummaryEntryData {
                    name: "NAR info timeout",
                    description: "Timeout in seconds for NAR info lookup requests.",
                    value: format!("{}s", cfg.nar_info_timeout.as_secs()),
                },
                ConfigSummaryEntryData {
                    name: "NAR timeout",
                    description: "Timeout in seconds for NAR file downloads, also used as connect timeout.",
                    value: format!("{}s", cfg.nar_timeout.as_secs()),
                },
                ConfigSummaryEntryData {
                    name: "Max concurrent requests",
                    description: "Maximum concurrent outgoing NAR streaming requests, applied per distinct substituter host.",
                    value: cfg.max_concurrent_requests.to_string(),
                },
                ConfigSummaryEntryData {
                    name: "Chunked streaming",
                    description: "Downloads NAR files via concurrent multi-connection chunked transfer if supported.",
                    value: cfg.chunked_streaming.to_string(),
                },
                ConfigSummaryEntryData {
                    name: "Streaming chunk max length",
                    description: "Maximum size in bytes of each chunk when downloading NAR files.",
                    value: format!("{} B", cfg.streaming_chunk_max_len.get()),
                },
                ConfigSummaryEntryData {
                    name: "Streaming window max length",
                    description: "Maximum number of chunks that may be in flight simultaneously for a single NAR file.",
                    value: cfg.streaming_window_max_len.get().to_string(),
                },
                ConfigSummaryEntryData {
                    name: "Periodic probing",
                    description: "Continuously probes substituters every 30 seconds to detect failures early.",
                    value: (cfg.periodic_probing == PeriodicProbingOption::Enabled).to_string(),
                },
                ConfigSummaryEntryData {
                    name: "Ignore NAR info error",
                    description: "NAR info lookup errors are treated as not-found instead of infrastructure errors.",
                    value: cfg.ignore_nar_info_error.to_string(),
                },
            ],
        }
    }

    fn proxy_section(&self) -> ConfigSummarySectionData {
        let cfg = &self.config.proxy;
        ConfigSummarySectionData {
            title: "Proxy",
            entries: vec![
                ConfigSummaryEntryData {
                    name: "Rewrite NAR URL",
                    description: "When enabled, the URL field in NAR info responses is rewritten according to the rewrite target.",
                    value: (cfg.rewrite_nar_url != NarUrlRewriteOption::Keep).to_string(),
                },
                ConfigSummaryEntryData {
                    name: "Rewrite target",
                    description: "Controls how the URL field is rewritten when rewrite NAR URL is enabled.",
                    value: match cfg.rewrite_nar_url {
                        NarUrlRewriteOption::Keep => "disabled",
                        NarUrlRewriteOption::ToSelf => "self",
                        NarUrlRewriteOption::ToUpstream => "upstream",
                    }
                    .to_string(),
                },
            ],
        }
    }

    fn cache_info_section(&self) -> ConfigSummarySectionData {
        let cfg = &self.config.cache_info;
        ConfigSummarySectionData {
            title: "Nix Cache Info",
            entries: vec![
                ConfigSummaryEntryData {
                    name: "StoreDir",
                    description: "Nix store directory path. Must be an absolute path.",
                    value: cfg.store_dir.clone(),
                },
                ConfigSummaryEntryData {
                    name: "WantMassQuery",
                    description: "Whether to advertise support for mass queries.",
                    value: cfg.want_mass_query.to_string(),
                },
                ConfigSummaryEntryData {
                    name: "Priority",
                    description: "Substituter priority advertised to Nix clients.",
                    value: cfg.priority.value().to_string(),
                },
            ],
        }
    }
}
