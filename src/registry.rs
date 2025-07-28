//! Type registry for dynamic component registration

use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::api::{Node, Policy, Router, Transport};
use crate::error::{Result, RatNetError};

/// Factory function type for creating routers
pub type RouterFactory = Box<dyn Fn(HashMap<String, Value>) -> Arc<dyn Router> + Send + Sync>;

/// Factory function type for creating policies
pub type PolicyFactory = Box<dyn Fn(Arc<dyn Transport>, Arc<dyn Node>, HashMap<String, Value>) -> Arc<dyn Policy> + Send + Sync>;

/// Factory function type for creating transports
pub type TransportFactory = Box<dyn Fn(Arc<dyn Node>, HashMap<String, Value>) -> Arc<dyn Transport> + Send + Sync>;

/// Global type registry for RatNet components
pub struct Registry {
    /// Registry of available Router modules by name
    routers: DashMap<String, RouterFactory>,
    
    /// Registry of available Policy modules by name
    policies: DashMap<String, PolicyFactory>,
    
    /// Registry of available Transport modules by name
    transports: DashMap<String, TransportFactory>,
}

impl Registry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            routers: DashMap::new(),
            policies: DashMap::new(),
            transports: DashMap::new(),
        }
    }
    
    /// Register a router factory
    pub fn register_router<F>(&self, name: String, factory: F)
    where
        F: Fn(HashMap<String, Value>) -> Arc<dyn Router> + Send + Sync + 'static,
    {
        self.routers.insert(name, Box::new(factory));
    }
    
    /// Register a policy factory
    pub fn register_policy<F>(&self, name: String, factory: F)
    where
        F: Fn(Arc<dyn Transport>, Arc<dyn Node>, HashMap<String, Value>) -> Arc<dyn Policy> + Send + Sync + 'static,
    {
        self.policies.insert(name, Box::new(factory));
    }
    
    /// Register a transport factory
    pub fn register_transport<F>(&self, name: String, factory: F)
    where
        F: Fn(Arc<dyn Node>, HashMap<String, Value>) -> Arc<dyn Transport> + Send + Sync + 'static,
    {
        self.transports.insert(name, Box::new(factory));
    }
    
    /// Create a new router instance from a configuration map
    pub fn new_router_from_map(&self, config: HashMap<String, Value>) -> Result<Arc<dyn Router>> {
        let router_type = config.get("Router")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RatNetError::InvalidArgument("Missing or invalid Router type".to_string()))?;
        
        let factory = self.routers.get(router_type)
            .ok_or_else(|| RatNetError::NotFound(format!("Router type '{}' not found", router_type)))?;
        
        Ok(factory(config))
    }
    
    /// Create a new policy instance from a configuration map
    pub fn new_policy_from_map(
        &self,
        transport: Arc<dyn Transport>,
        node: Arc<dyn Node>,
        config: HashMap<String, Value>,
    ) -> Result<Arc<dyn Policy>> {
        let policy_type = config.get("Policy")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RatNetError::InvalidArgument("Missing or invalid Policy type".to_string()))?;
        
        let factory = self.policies.get(policy_type)
            .ok_or_else(|| RatNetError::NotFound(format!("Policy type '{}' not found", policy_type)))?;
        
        Ok(factory(transport, node, config))
    }
    
    /// Create a new transport instance from a configuration map
    pub fn new_transport_from_map(
        &self,
        node: Arc<dyn Node>,
        config: HashMap<String, Value>,
    ) -> Result<Arc<dyn Transport>> {
        let transport_type = config.get("Transport")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RatNetError::InvalidArgument("Missing or invalid Transport type".to_string()))?;
        
        let factory = self.transports.get(transport_type)
            .ok_or_else(|| RatNetError::NotFound(format!("Transport type '{}' not found", transport_type)))?;
        
        Ok(factory(node, config))
    }
    
    /// Get list of registered router types
    pub fn get_router_types(&self) -> Vec<String> {
        self.routers.iter().map(|entry| entry.key().clone()).collect()
    }
    
    /// Get list of registered policy types
    pub fn get_policy_types(&self) -> Vec<String> {
        self.policies.iter().map(|entry| entry.key().clone()).collect()
    }
    
    /// Get list of registered transport types
    pub fn get_transport_types(&self) -> Vec<String> {
        self.transports.iter().map(|entry| entry.key().clone()).collect()
    }
    
    /// Clear all registrations (useful for testing)
    pub fn clear(&self) {
        self.routers.clear();
        self.policies.clear();
        self.transports.clear();
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience macros for registration
#[macro_export]
macro_rules! register_router {
    ($name:expr, $factory:expr) => {
        $crate::REGISTRY.register_router($name.to_string(), $factory);
    };
}

#[macro_export]
macro_rules! register_policy {
    ($name:expr, $factory:expr) => {
        $crate::REGISTRY.register_policy($name.to_string(), $factory);
    };
}

#[macro_export]
macro_rules! register_transport {
    ($name:expr, $factory:expr) => {
        $crate::REGISTRY.register_transport($name.to_string(), $factory);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_registry_creation() {
        let registry = Registry::new();
        assert!(registry.get_router_types().is_empty());
        assert!(registry.get_policy_types().is_empty());
        assert!(registry.get_transport_types().is_empty());
    }
    
    #[test]
    fn test_missing_type_error() {
        let registry = Registry::new();
        let config = HashMap::new();
        
        let result = registry.new_router_from_map(config);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            RatNetError::InvalidArgument(msg) => {
                assert!(msg.contains("Missing or invalid Router type"));
            }
            _ => panic!("Expected InvalidArgument error"),
        }
    }
} 