use std::sync::{Arc, RwLock};
use once_cell::sync::OnceCell;
use crate::models::CitationGraph;
use crate::algorithm::PageRankCalculator;

pub static APP_STATE: OnceCell<Arc<AppState>> = OnceCell::new();

#[derive(Debug)]
pub struct AppState {
    pub graph: Arc<RwLock<CitationGraph>>,
    pub last_update: Arc<RwLock<chrono::DateTime<chrono::Utc>>>,
    pub cache: Arc<RwLock<Cache>>,
}

#[derive(Debug)]
pub struct Cache {
    pub top_venues: Option<Vec<crate::models::VenueRanking>>,
    pub top_scholars: Option<Vec<crate::models::ScholarRanking>>,
    pub last_refresh: chrono::DateTime<chrono::Utc>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            top_venues: None,
            top_scholars: None,
            last_refresh: chrono::Utc::now(),
        }
    }
    
    pub fn is_stale(&self) -> bool {
        let now = chrono::Utc::now();
        (now - self.last_refresh).num_minutes() > 5
    }
    
    pub fn invalidate(&mut self) {
        self.top_venues = None;
        self.top_scholars = None;
    }
    
    pub fn refresh(&mut self) {
        self.last_refresh = chrono::Utc::now();
    }
}

impl AppState {
    pub fn new(graph: CitationGraph) -> Self {
        Self {
            graph: Arc::new(RwLock::new(graph)),
            last_update: Arc::new(RwLock::new(chrono::Utc::now())),
            cache: Arc::new(RwLock::new(Cache::new())),
        }
    }
    
    pub fn get_or_calculate_venues(&self, top: usize) -> Vec<crate::models::VenueRanking> {
        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if !cache.is_stale() {
                if let Some(ref venues) = cache.top_venues {
                    if venues.len() >= top {
                        return venues[..top].to_vec();
                    }
                }
            }
        }
        
        // Calculate if not in cache
        let graph = self.graph.read().unwrap();
        let mut calculator = PageRankCalculator::new(&*graph);
        calculator.calculate().ok();
        let venues = calculator.get_top_venues(100, None, None);
        
        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.top_venues = Some(venues.clone());
            cache.refresh();
        }
        
        venues.into_iter().take(top).collect()
    }
    
    pub fn get_or_calculate_scholars(&self, top: usize) -> Vec<crate::models::ScholarRanking> {
        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if !cache.is_stale() {
                if let Some(ref scholars) = cache.top_scholars {
                    if scholars.len() >= top {
                        return scholars[..top].to_vec();
                    }
                }
            }
        }
        
        // Calculate if not in cache
        let graph = self.graph.read().unwrap();
        let mut calculator = PageRankCalculator::new(&*graph);
        calculator.calculate().ok();
        let scholars = calculator.get_top_scholars(100, None);
        
        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.top_scholars = Some(scholars.clone());
            cache.refresh();
        }
        
        scholars.into_iter().take(top).collect()
    }
}