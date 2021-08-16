#![feature(fn_traits)]
//! # Singleton Manager
//! A singleton manger for handling and holding singletons in a system
//!
//! ## build Purpose
//! This was build because of multiple minor libraries that was either using `lazy_static!` or other
//! macros to handle late initializations of singletons in system.
//!
//! Example of such libraries
//!
//! 1. Database Pool Manager, that though out the entire services lifetime needs to continue running to track the number of active connections.
//! 2. Logging, a logging library that collects and tracks calls through out the entire execution of a single thread, for then later to flush all the logs as a collection of a single request.
//! 3. Worker Queue system, a worker queue system that needs to track each individual execution, and where new executions can be dynamically added to it while running.
//!
//! Previously the applications were using `lazy_static!` but `lazy_static!` is using `unsafe` modify
//! on each activation. To reduce the failurepoints this system is also using `unsafe` but only in
//! one place to minimize impact, on top of that it is programmatically accessible and modifiable.
//! Allowing you to create object on the fly when needed.
//!
//!
//! A full example of how to use this:
//! ```
//! use singleton_manager::sm;
//! use std::sync::Mutex;
//!
//! pub struct MyService {
//!     message: String,
//!     guard: Mutex<()>,
//! }
//!
//! impl MyService {
//!     pub fn set(&mut self, msg: &str) {
//!         let mut _guard = self.guard.lock().expect("Failed to get guard");
//!         self.message = msg.to_string();
//!     }
//!
//!     pub fn get(&self) -> String {
//!         let _guard = self.guard.lock();
//!         self.message.clone()
//!     }
//! }
//!
//! sm().set("my_service",
//!     MyService {
//!         message: "".to_string(),
//!         guard: Mutex::new(()),
//!     }).ok();
//!
//! let service = sm()
//!     .get::<MyService>("my_service")
//!     .expect("Failed to get service");
//! service.set("My Message");
//!
//! let different_service = sm()
//!     .get::<MyService>("my_service")
//!     .expect("Failed to get service");
//!
//!assert_eq!("My Message".to_string(), different_service.get());
//! ```
extern crate uuid;

use std::any::Any;
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::ops::DerefMut;
use std::sync::Once;
use uuid::Uuid;

static mut INSTANCE: Cell<Option<SingletonManager>> = Cell::new(None);
static mut ONCE: Once = Once::new();

/// Common Result used in the library.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum Error {
    ServiceDoesNotExist(String),
    ServiceNotInstantiated(String),
    FailedToDowncastRefOfService(String),
    FailedToStoreService(String),
    NoFactoryFunctionAvailable(String),
    SetFailedToReturnAServiceReference(String),
    FailedToDowncastFactoryOutput(String),
    NoServiceWithStorageRequest,
    FailedToStoreServiceAlias,
    MutexGotPoison,
    ServiceAlreadyExists,
    FailedToStoreFactory,
    UnknownError(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::UnknownError(s)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServiceDoesNotExist(ref s) => write!(f, "Service `{}` does not exist", s),
            Self::ServiceNotInstantiated(ref s) => write!(f, "Service `{}` is not instantiated", s),
            Self::FailedToDowncastRefOfService(ref s) => {
                write!(f, "Failed to downcast service {}", s)
            }
            Self::FailedToStoreService(ref s) => write!(f, "Service `{}` Could not be stored", s),
            Self::NoFactoryFunctionAvailable(ref s) => {
                write!(f, "Service `{}` Does not contain a Factory function", s)
            }
            Self::SetFailedToReturnAServiceReference(ref s) => write!(
                f,
                "Storing the service `{}` failed to return a reference of the service",
                s
            ),
            Self::FailedToDowncastFactoryOutput(ref s) => {
                write!(f, "Failed to downcast Factory output for service {}", s)
            }

            Self::NoServiceWithStorageRequest => write!(f, "No service with storage request"),
            Self::FailedToStoreServiceAlias => write!(f, "Service Could not be stored"),
            Self::MutexGotPoison => write!(f, "Mutex poison"),
            Self::ServiceAlreadyExists => write!(f, "Service already exists"),
            Self::FailedToStoreFactory => write!(f, "Failed to store factory"),
            Self::UnknownError(s) => write!(f, "An unknown error happened: {}", s),
        }
    }
}

/// setting up the support for std::error::Error to allow error handling and passthroughs
/// ```
/// pub enum SomeError {
///     CustomError(Box<dyn std::error::Error + Send>)
/// }
/// ```
/// The concept of this is that it will allow for either alter parsing of the Error value later
/// without the loss of information.
impl std::error::Error for Error {}

/// Singleton Manager
/// The container of the singleton managers information.
/// This allows to set aliases to lookup the stored singleton, and allowing for creating a factory
/// function to be set. In the case that the Singleton is never used the factory will stay dormant.
///
pub struct SingletonManager {
    /// The singleton for the "service" or structure that needs a singular instantiation.
    singletons: HashMap<Uuid, Box<dyn Any>>,
    /// A factory function that can be used for creating the singleton
    singleton_factories: HashMap<Uuid, Box<dyn Fn() -> Box<dyn Any>>>,
    // instance_type: HashMap<Uuid, String>,
    /// Alias for the actual Singleton. This is linking an actual name to the singleton storage.
    alias: HashMap<String, Uuid>,
}

impl SingletonManager {
    fn new() -> SingletonManager {
        SingletonManager {
            singletons: HashMap::new(),
            singleton_factories: HashMap::new(),
            // instance_type: HashMap::new(),
            alias: HashMap::new(),
        }
    }

    /// Getting the instance of the SigneltonManager
    /// This will return a static reference to the singleton manager.
    /// ```
    /// use singleton_manager::SingletonManager;
    ///
    /// let sm = SingletonManager::instance();
    /// ```
    /// A simple way to get the singleton manager
    pub fn instance() -> &'static mut SingletonManager {
        unsafe {
            ONCE.call_once(|| INSTANCE = Cell::new(Some(SingletonManager::new())));
            match *INSTANCE.as_ptr() {
                Some(ref mut messenger) => messenger,
                None => panic!("Failed to get instance"),
            }
        }
    }

    /// Implementation of provider sets
    ///
    /// > NOTE:
    /// > This is currently being implemented and the expectation is that the singleton manager
    /// > will be moving more and more towards using the provider implementation in the future.
    ///
    /// This allows you to implement a `SingletonProvider` trait for a `struct` and form there just
    /// service the implemented struct to the Singleton Manager.
    /// The Singleton Manager can then on a need to need basis either get the singleton from its own
    /// storage or create the service in its own storage before serving it. Giving the
    /// Singleton Manager total access over when a service should be created and reused.
    ///
    /// Usage:
    /// ```
    /// use singleton_manager::{SingletonManager, SingletonProvider};
    /// use std::sync::Mutex;
    ///
    /// struct MyService{
    ///     message: String,
    ///     guard: Mutex<()>
    /// };
    ///
    /// impl MyService {
    ///     pub fn set(&mut self, msg: &str) {
    ///         let mut _guard = self.guard.lock().expect("Failed to get guard");
    ///         self.message = msg.to_string();
    ///     }
    ///
    ///     pub fn get(&self) -> String {
    ///         let _guard = self.guard.lock();
    ///         self.message.clone()
    ///     }
    /// }
    ///
    /// impl SingletonProvider for MyService {
    ///     type Output = MyService;
    ///     type Error = String;
    ///
    ///     fn service() -> Result<&'static mut Self::Output, Self::Error> {
    ///         SingletonManager::instance().get::<Self::Output>("my_service").map_err(|_| "err".to_string())
    ///     }
    ///
    ///     fn get_name(&self) -> &'static str {
    ///         "my_service"
    ///     }
    ///
    ///     fn get_service(&self) -> Result<Self::Output, Self::Error> {
    ///         Ok(MyService{
    ///             message: "".to_string(),
    ///             guard: Mutex::new(()),
    ///         })
    ///     }
    /// }
    ///
    /// SingletonManager::instance().provide(MyService {
    ///     message: "".to_string(),
    ///     guard: Mutex::new(()),
    /// });
    /// ```
    pub fn provide(&'static mut self, sp: impl SingletonProvider) -> Result<()> {
        let t = sp.get_service().map_err(|e| e.into())?;
        self.set(sp.get_name(), t).map(|_| ())
    }

    /// get with default,
    ///
    /// This will get a singleton from the singleton manager.
    /// If the singleton does not exist it will automatically create it from the default factory
    /// function and then store the build singleton.
    ///
    pub fn get_default<T: 'static, F: 'static>(
        &self,
        service_name: &str,
        factory: F,
    ) -> Result<&'static mut T>
    where
        F: Fn() -> Box<dyn Any>,
    {
        if !self.has(service_name) {
            SingletonManager::instance()
                .set_factory(service_name, factory)
                .ok();
        }
        sm().get::<T>(service_name)
    }

    pub fn has(&self, service_name: &str) -> bool {
        self.alias.contains_key(service_name)
    }

    /// Getting a singleton from the singleton manager.
    /// This allow you to get a certain singleton from the singleton manager.
    /// This will automatically try to downcast the singleton to the expected object, if the
    /// downcast failes it will return an Error `FailedToDowncastRefOfService([Service_name])`
    /// to let you know that the downcast failed for the sytsem.
    ///
    /// To use this just use the following code:
    ///
    /// ```
    /// use singleton_manager::SingletonManager;
    /// use singleton_manager::sm;
    ///
    /// struct MyService{};
    ///
    /// sm().set("my_service", MyService {}).unwrap();
    ///
    /// let service = sm().get::<MyService>("my_service")
    ///     .expect("Failed to get service");
    /// ```
    ///
    /// this will give you the `my_service` that have been set previously.
    /// A full example of its usage can be found here:
    pub fn get<T: 'static>(&'static mut self, service_name: &str) -> Result<&'static mut T> {
        SingletonManager::instance()
            .alias
            .get(service_name)
            .ok_or_else(|| Error::ServiceDoesNotExist(service_name.to_string()))
            .and_then(|id| sm().singleton_get(id))
            .and_then(|service_box| {
                service_box
                    .downcast_mut::<T>()
                    .ok_or_else(|| Error::FailedToDowncastRefOfService(service_name.to_string()))
            })
    }

    /// Setting a specific service/object as a singleton.
    /// This is used when setting a service or other to a singleton.
    pub fn set<T: 'static>(&self, service_name: &str, service: T) -> Result<&'static mut T> {
        sm().store_alias(service_name).and_then(|id| {
            sm().singleton_set(id, Box::new(service))
                .and_then(|service_box| {
                    service_box.downcast_mut::<T>().ok_or_else(|| {
                        Error::FailedToDowncastRefOfService(service_name.to_string())
                    })
                })
        })
    }

    pub fn set_factory<F: 'static + Fn() -> Box<dyn Any>>(
        &self,
        service_name: &str,
        factory: F,
    ) -> Result<&'static mut Box<dyn Fn() -> Box<dyn Any>>> {
        sm().store_alias(service_name)
            .and_then(|id| sm().singleton_factory_set(&id, Box::new(factory)))
    }

    fn store_alias(&self, alias: &str) -> Result<Uuid> {
        if sm().alias.contains_key(alias) {
            Err(Error::ServiceAlreadyExists)
        } else {
            sm().alias.insert(alias.to_string(), Uuid::new_v4());
            if let Some(id) = sm().alias.get(alias) {
                Ok(*id)
            } else {
                Err(Error::FailedToStoreServiceAlias)
            }
        }
    }

    fn singleton_get(&'static mut self, alias: &Uuid) -> Result<&mut Box<dyn Any>> {
        sm().singletons
            .get_mut(alias)
            .ok_or_else(|| Error::ServiceDoesNotExist(alias.to_string()))
            .or_else(|_| {
                if sm().singleton_factories.contains_key(alias) {
                    sm().factory(alias)
                } else {
                    Err(Error::ServiceDoesNotExist(alias.to_string()))
                }
            })
    }

    fn singleton_set(&self, id: Uuid, service: Box<dyn Any>) -> Result<&'static mut Box<dyn Any>> {
        sm().singletons.insert(id, service);
        if sm().singletons.contains_key(&id) {
            sm().singletons
                .get_mut(&id)
                .ok_or_else(|| Error::FailedToStoreService(id.to_string()))
        } else {
            Err(Error::ServiceAlreadyExists)
        }
    }

    fn singleton_factory_set<F: 'static + Fn() -> Box<dyn Any>>(
        &self,
        id: &Uuid,
        factory: Box<F>,
    ) -> Result<&'static mut Box<dyn Fn() -> Box<dyn Any>>> {
        sm().singleton_factories.insert(*id, factory);
        if self.singleton_factories.contains_key(&id) {
            sm().singleton_factories
                .get_mut(&id)
                .ok_or(Error::FailedToStoreFactory)
        } else {
            Err(Error::FailedToStoreFactory)
        }
    }

    fn factory(&'static mut self, alias: &Uuid) -> Result<&mut Box<dyn Any>> {
        if let Some(box_func) = self.singleton_factories.get_mut(alias) {
            sm().execute_factory(box_func)
                .map(|service| self.singletons.insert(*alias, service))
                .ok();
            if self.singletons.contains_key(alias) {
                sm().singletons
                    .get_mut(alias)
                    .ok_or_else(|| Error::ServiceDoesNotExist(alias.to_string()))
            } else {
                Err(Error::ServiceDoesNotExist(alias.to_string()))
            }
        } else {
            Err(Error::NoFactoryFunctionAvailable(alias.to_string()))
        }
    }

    fn execute_factory(
        &'static mut self,
        factory: &mut Box<dyn Fn() -> Box<dyn Any>>,
    ) -> Result<Box<dyn Any>> {
        let func = factory.deref_mut();
        let service = func();
        Ok(service)
    }

    // fn get_alias(&'static self, alias: &str) -> Result<&Uuid, Error> {
    //     self.alias
    //         .get(alias)
    //         .ok_or(Error::ServiceDoesNotExist(alias.to_string()))
    // }
}

pub trait SingletonProvider {
    type Output: 'static;
    type Error: Into<Error>;
    fn service() -> std::result::Result<&'static mut Self::Output, Self::Error>;
    fn get_name(&self) -> &'static str;
    fn get_service(&self) -> std::result::Result<Self::Output, Self::Error>;
}

pub fn sm() -> &'static mut SingletonManager {
    SingletonManager::instance()
}
pub fn singleton_manager() -> &'static mut SingletonManager {
    SingletonManager::instance()
}
// pub fn set_factory<T: 'static>(&self, service_name: &str, factory: T) -> Result<(), String> {}

#[cfg(test)]
mod test {
    use super::SingletonManager;

    use std::ops::Deref;
    use std::sync::Mutex;

    struct SingletonService1 {
        something: String,
    }

    #[derive(Debug)]
    pub struct MyService {
        message: String,
        guard: Mutex<()>,
    }

    impl MyService {
        pub fn set(&mut self, msg: &str) {
            let mut _guard = self.guard.lock().expect("Failed to get guard");
            self.message = msg.to_string();
        }

        pub fn get(&self) -> String {
            let _guard = self.guard.lock();
            self.message.clone()
        }
    }

    #[test]
    pub fn test_instance() {
        SingletonManager::instance();
    }

    #[test]
    pub fn set_singleton() {
        SingletonManager::instance()
            .set(
                "my_service_0",
                Box::new(SingletonService1 {
                    something: "hello".to_string(),
                }),
            )
            .unwrap();
    }

    #[test]
    pub fn set_get_singleton() {
        SingletonManager::instance()
            .set(
                "my_service_1",
                SingletonService1 {
                    something: "hello".to_string(),
                },
            )
            .unwrap();
        let var = SingletonManager::instance()
            .get::<SingletonService1>("my_service_1")
            .unwrap()
            .something
            .clone();

        assert_eq!("hello".to_string(), var);
    }

    #[test]
    #[warn(clippy::unnecessary_operation)]
    fn test_downcast() {
        let instance_name = "MyService";
        let service_name = "my_downcast_test";
        let my_function = Some(Box::new(|| {
            Box::new(MyService {
                message: "".to_string(),
                guard: Mutex::new(()),
            })
        }));
        my_function
            .as_ref()
            .ok_or_else(|| super::Error::NoFactoryFunctionAvailable(service_name.to_string()))
            .map(|f| (instance_name, f))
            .map(|(instance, factory)| {
                let func = factory.deref();
                let output = func.call(());
                (instance, output as Box<MyService>)
            })
            .map(|(_instance_name, service)| service)
            .map(|s| println!("{:?}", s))
            .ok();
    }

    #[test]
    fn test_setting_and_getting_from_example() {
        SingletonManager::instance()
            .set(
                "my_service",
                MyService {
                    message: "".to_string(),
                    guard: Mutex::new(()),
                },
            )
            .ok();

        let service = SingletonManager::instance()
            .get::<MyService>("my_service")
            .expect("Failed to get service");
        service.set("My Message");

        let different_service = SingletonManager::instance()
            .get::<MyService>("my_service")
            .expect("Failed to get service");
        assert_eq!("My Message".to_string(), different_service.get());
    }

    #[test]
    fn test_setting_and_getting_from_example_factory() {
        SingletonManager::instance()
            .set_factory("my_service_factory", || {
                Box::new(MyService {
                    message: "".to_string(),
                    guard: Mutex::new(()),
                })
            })
            .ok();

        let service = SingletonManager::instance()
            .get::<MyService>("my_service_factory")
            .unwrap();
        service.set("My Message");

        let different_service = SingletonManager::instance()
            .get::<MyService>("my_service_factory")
            .unwrap();
        assert_eq!("My Message".to_string(), different_service.get());
    }

    #[test]
    fn test_setting_and_getting_from_example_default_factory() {
        let service: &mut MyService = SingletonManager::instance()
            .get_default("my_default_service_factory", || {
                Box::new(MyService {
                    message: "".to_string(),
                    guard: Mutex::new(()),
                })
            })
            .unwrap();
        service.set("My Message");

        assert_eq!("My Message".to_string(), service.get());
    }
}
