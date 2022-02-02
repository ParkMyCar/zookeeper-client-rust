# ZooKeeper client in async rust

## Examples
```rust
use std::time::Duration;

use zookeeper_client as zk;

let path = "/abc";
let data = "path_data".as_bytes().to_vec();
let child_path = "/abc/efg";
let child_data = "child_path_data".as_bytes().to_vec();
let create_options = zk::CreateOptions::new(zk::CreateMode::Persistent, zk::Acl::anyone_all());

let cluster = "localhost:2181";
let client = zk::Client::connect(cluster, Duration::from_secs(20)).await.unwrap();
let (_, stat_watcher) = client.check_and_watch_stat(path).await.unwrap();

let (stat, _) = client.create(path, &data, &create_options).await.unwrap();
assert_eq!((data.clone(), stat), client.get_data(path).await.unwrap());

let event = stat_watcher.changed().await;
assert_eq!(event.event_type, zk::EventType::NodeCreated);
assert_eq!(event.path, path);

let path_client = client.clone().chroot(path).unwrap();
assert_eq!((data, stat), path_client.get_data("/").await.unwrap());

let (_, _, child_watcher) = client.get_and_watch_children(path).await.unwrap();

let (child_stat, _) = client.create(child_path, &child_data, &create_options).await.unwrap();

let child_event = child_watcher.changed().await;
assert_eq!(child_event.event_type, zk::EventType::NodeChildrenChanged);
assert_eq!(child_event.path, path);

let relative_child_path = child_path.strip_prefix(path).unwrap();
assert_eq!((child_data.clone(), child_stat), path_client.get_data(relative_child_path).await.unwrap());

let (_, _, event_watcher) = client.get_and_watch_data("/").await.unwrap();
drop(client);
drop(path_client);

let session_event = event_watcher.changed().await;
assert_eq!(session_event.event_type, zk::EventType::Session);
assert_eq!(session_event.session_state, zk::SessionState::Closed);
```

For more examples, see [zookeeper.rs](tests/zookeeper.rs).

## TODO
[ ] Drop for PersistentWatcher
[ ] Sasl authentication

## License
The MIT License (MIT). See [LICENSE](LICENSE) for the full license text.