# Singleton Manager

Is a management service for Singleton Services.
If there is a need for a service to keep its state across multiple threads,
this singleton service can pull the services from a singleton storage.

> Note:
> Singleton Manager itself is not thread-safe, so you need to make your service threadsafe.

## Usage
Say we want to use a custom `struct` as our singleton:
```rust
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
```

### Using object instance
if this is our service and we would now want to store it as a singleton, we can do this simply by:

```rust
SingletonManager::instance().set(
        "my_service",
        "MyService",
        None,
        Some(MyService {
            message: "".to_string(),
            guard: Mutex::new(()),
        }),
    )
    .ok();
```

this will set the service and is now retrivable by using:

```rust
    let service = SingletonManager::instance()
        .get::<MyService>("my_service")
        .expect("Failed to get service");
    service.set("My Message");
    
    let different_service = SingletonManager::instance()
        .get::<MyService>("my_service")
        .expect("Failed to get service");
    assert_eq!("My Message".to_string(), different_service.get());
```
#### Full example
```rust
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

fn main() {
    SingletonManager::instance().set(
        "my_service",
        "MyService",
        None,
        Some(MyService {
            message: "".to_string(),
            guard: Mutex::new(()),
        }),
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
```
### Using factory instance
It's possible to instantiate only on request by using a factory to create the service

```rust
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
```

this will set the service and is now retrivable by using:

```rust
fn main() {
    set_service();
    let service = SingletonManager::instance()
        .get::<MyService>("my_service_factory")
        .expect("Failed to get service");
    service.set("My Message");
    
    let different_service = SingletonManager::instance()
        .get::<MyService>("my_service_factory")
        .expect("Failed to get service");
    assert_eq!("My Message".to_string(), different_service.get());
}
```
#### Full Example
```rust
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

fn main() {
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
```

## Contributions/Issues
Contributions are currently not opened as this is running from a private server.
Issues can be opened at any time with a guest account on gitlab.nebula.technology.
