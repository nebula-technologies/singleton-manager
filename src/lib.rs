#![feature(fn_traits)]
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
        }
    }
}

impl std::error::Error for Error {}

pub struct SingletonManager {
    singletons: HashMap<Uuid, Box<dyn Any>>,
    singleton_factories: HashMap<Uuid, Box<dyn Fn() -> Box<dyn Any>>>,
    // instance_type: HashMap<Uuid, String>,
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

    pub fn instance() -> &'static mut SingletonManager {
        unsafe {
            ONCE.call_once(|| INSTANCE = Cell::new(Some(SingletonManager::new())));
            match *INSTANCE.as_ptr() {
                Some(ref mut messenger) => messenger,
                None => panic!("Failed to get instance"),
            }
        }
    }

    pub fn get_default<T: 'static, F: Fn() -> T>(
        &'static mut self,
        service_name: &str,
        factory: Box<dyn Fn() -> Box<dyn Any>>,
    ) -> Result<&'static mut T, Error> {
        SingletonManager::instance()
            .alias
            .get(service_name)
            .ok_or(Error::ServiceDoesNotExist(service_name.to_string()))
            .and_then(|id| {
                smi().singleton_factory_set(id, factory).ok();
                smi().get::<T>(service_name)
            })
    }

    pub fn get<T: 'static>(&'static mut self, service_name: &str) -> Result<&'static mut T, Error> {
        SingletonManager::instance()
            .alias
            .get(service_name)
            .ok_or(Error::ServiceDoesNotExist(service_name.to_string()))
            .and_then(|id| smi().singleton_get(id))
            .and_then(|service_box| {
                service_box
                    .downcast_mut::<T>()
                    .ok_or_else(|| Error::FailedToDowncastRefOfService(service_name.to_string()))
            })
    }

    pub fn set<T: 'static>(&self, service_name: &str, service: T) -> Result<&'static mut T, Error> {
        smi().store_alias(service_name).and_then(|id| {
            smi()
                .singleton_set(id, Box::new(service))
                .and_then(|service_box| {
                    service_box.downcast_mut::<T>().ok_or_else(|| {
                        Error::FailedToDowncastRefOfService(service_name.to_string())
                    })
                })
        })
    }

    pub fn set_factory(
        &self,
        service_name: &str,
        factory: Box<dyn Fn() -> Box<dyn Any>>,
    ) -> Result<&'static mut Box<dyn Fn() -> Box<dyn Any>>, Error> {
        smi()
            .store_alias(service_name)
            .and_then(|id| smi().singleton_factory_set(&id, factory))
    }

    fn store_alias(&self, alias: &str) -> Result<Uuid, Error> {
        if smi().alias.contains_key(alias) {
            Err(Error::ServiceAlreadyExists)
        } else {
            smi().alias.insert(alias.to_string(), Uuid::new_v4());
            if let Some(id) = smi().alias.get(alias) {
                Ok(*id)
            } else {
                Err(Error::FailedToStoreServiceAlias)
            }
        }
    }

    fn singleton_get(&'static mut self, alias: &Uuid) -> Result<&mut Box<dyn Any>, Error> {
        smi()
            .singletons
            .get_mut(alias)
            .ok_or_else(|| Error::ServiceDoesNotExist(alias.to_string()))
            .or_else(|_| {
                if smi().singleton_factories.contains_key(alias) {
                    smi().factory(alias)
                } else {
                    Err(Error::ServiceDoesNotExist(alias.to_string()))
                }
            })
    }

    fn singleton_set(
        &self,
        id: Uuid,
        service: Box<dyn Any>,
    ) -> Result<&'static mut Box<dyn Any>, Error> {
        smi().singletons.insert(id, service);
        if smi().singletons.contains_key(&id) {
            smi()
                .singletons
                .get_mut(&id)
                .ok_or_else(|| Error::FailedToStoreService(id.to_string()))
        } else {
            Err(Error::ServiceAlreadyExists)
        }
    }

    fn singleton_factory_set(
        &self,
        id: &Uuid,
        factory: Box<dyn Fn() -> Box<dyn Any>>,
    ) -> Result<&'static mut Box<dyn Fn() -> Box<dyn Any>>, Error> {
        smi().singleton_factories.insert(id.clone(), factory);
        if self.singleton_factories.contains_key(&id) {
            smi()
                .singleton_factories
                .get_mut(&id)
                .ok_or_else(|| Error::FailedToStoreFactory)
        } else {
            Err(Error::FailedToStoreFactory)
        }
    }

    fn factory(&'static mut self, alias: &Uuid) -> Result<&mut Box<dyn Any>, Error> {
        if let Some(box_func) = self.singleton_factories.get_mut(alias) {
            smi()
                .execute_factory(box_func)
                .map(|service| self.singletons.insert(alias.clone(), service))
                .ok();
            if self.singletons.contains_key(alias) {
                smi()
                    .singletons
                    .get_mut(alias)
                    .ok_or(Error::ServiceDoesNotExist(alias.to_string()))
            } else {
                Err(Error::ServiceDoesNotExist(alias.to_string()))
            }
        } else {
            Err(Error::NoFactoryFunctionAvailable(alias.to_string()))
        }
    }

    fn execute_factory(
        &'static mut self,
        mut factory: &mut Box<dyn Fn() -> Box<dyn Any>>,
    ) -> Result<Box<dyn Any>, Error> {
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

fn smi() -> &'static mut SingletonManager {
    SingletonManager::instance()
}

// pub fn set_factory<T: 'static>(&self, service_name: &str, factory: T) -> Result<(), String> {}

#[cfg(test)]
mod test {
    use super::SingletonManager;

    use std::any::Any;
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
            .map(|(instance_name, service)| service)
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
            .set_factory(
                "my_service_factory",
                Box::new(|| {
                    Box::new(MyService {
                        message: "".to_string(),
                        guard: Mutex::new(()),
                    })
                }),
            )
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
}
