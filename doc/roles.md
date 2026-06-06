下面是一份可以直接保存为 `rust-code-review-checklist.md` 的 Markdown 文档。

# Rust 代码规范自检清单

适用场景：

* 审查 AI / Agent 生成的 Rust 代码
* Code Review 前自查
* 排查“能编译但不够 Rust”的代码
* 避免为了过编译器而滥用 `unwrap()`、`clone()`、`Arc<Mutex<_>>`、`'static`

核心原则：

> Rust 代码质量的关键，不只是“能不能编译”，而是有没有清楚表达：谁拥有数据、谁只是借用、错误如何传播、状态如何共享。

---

## 0. 行动纪律（Agent 必读，优先级高于后续所有条目）

本清单用于**审查与判断**，不是“看到坏味道就必须改”。后面每一条几乎都有“可接受场景”，请先判断再决定是否动手。把它当成提问清单，不是重构待办列表。

1. **本清单是判断标准，不是修改目标。**
   看到 `unwrap` / `clone` / `Arc<Mutex>` / `String` 参数 / `serde_json::Value` 等写法，先问“这里是否真的有问题”，确认有问题、且在当前任务范围内，才改。不要为了消除坏味道而制造无意义 diff。

2. **只改被明确要求的范围，不顺手重构无关代码。**
   修复 A 时不要顺手“优化”B、C。本清单里的风格建议，只在它直接服务于当前任务时才应用。

3. **下列改动属于高风险，动手前必须先 `rg` 确认所有调用方，并向用户确认：**
   - 修改公共 API / 函数签名（含 `String`→`&str`、`Vec<T>`→`&[T]`、改 trait 方法）
   - 删除“看起来无用”的代码、配置、feature flag
   - 拆分文件 / 调整模块结构
   - 修改字段可见性（`pub` → private + getter）
   - 重构错误类型（引入 `thiserror` / 自定义错误枚举）
   这些改动会连锁波及调用点，是 agent 最容易“改坏项目”的地方。

4. **改动前后都要有验证。**
   动手前先确认当前测试基线（哪些是绿的）。改完必须通过：
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   ```
   不要用“能 `cargo build`”代替“正确”。

5. **判断“无用”要保守。**
   宏、`pub` 导出、`cfg(feature)`、跨 crate 调用、运行时注册都可能让“看似无调用方”的代码其实在用。判不准就不要删，向用户报告。

> 一句话：本清单帮你*看出*问题，由你和用户决定*改不改*；牵连多文件的改动一律先列计划、经确认再执行。

---

## 1. `unwrap()` / `expect()` 自检

### 警惕写法

```rust
let value = option.unwrap();
let result = do_something().unwrap();
let config = load_config().expect("failed");
```

### 自检问题

看到 `unwrap()` 时，必须问：

1. 这里失败是否真的“不可能”？
2. 如果失败，是程序员 bug，还是正常业务分支？
3. 这段代码是在测试、demo、启动初始化，还是生产业务路径？
4. panic 是否会导致服务崩溃、请求失败、数据丢失？

### 可以接受的场景

#### 测试代码

```rust
let user = parse_user(input).unwrap();
```

#### 写死的常量，失败代表程序员写错

```rust
let re = Regex::new(r"^\d+$").unwrap();
```

#### 启动阶段必须成功，否则程序不能运行

```rust
let config = load_config().expect("failed to load config at startup");
```

### 更推荐的写法

#### 用 `?` 传播错误

```rust
let config = load_config()?;
```

#### 将 `Option` 转成 `Result`

```rust
let user = users.get(id).ok_or(Error::UserNotFound)?;
```

#### 显式处理不同情况

```rust
match users.get(id) {
    Some(user) => handle_user(user),
    None => return Err(Error::UserNotFound),
}
```

#### 提供默认值

```rust
let port = std::env::var("PORT")
    .unwrap_or_else(|_| "3000".to_string());
```

### 结论

`unwrap()` 的本质是：

```rust
// 我不处理失败，失败就崩
```

业务代码中，除非失败确实代表程序 bug，否则不要随便使用。

---

## 2. `clone()` 自检

### 警惕写法

```rust
foo(data.clone());
bar(data.clone());
baz(data.clone());
```

```rust
let name = user.name.clone();
```

### 自检问题

看到 `clone()` 时，必须问：

1. 这里是真的需要一份新数据，还是只是为了绕过 borrow checker？
2. 被 clone 的对象大不大？
3. clone 的是 `String`、`Vec<T>`、`HashMap<K, V>`，还是 `Arc<T>` 这种轻量引用计数？
4. 被调用函数是否只是读取数据？
5. 函数参数是否应该改成引用？

### 可疑写法

```rust
fn print_user(user: User) {
    println!("{}", user.name);
}

print_user(user.clone());
```

### 更推荐的写法

如果函数只是读取，不要拿 ownership：

```rust
fn print_user(user: &User) {
    println!("{}", user.name);
}

print_user(&user);
```

字符串参数优先用 `&str`：

```rust
fn greet(name: &str) {
    println!("hello {name}");
}

let name = String::from("Alice");
greet(&name);
greet("Bob");
```

集合参数优先用 slice：

```rust
fn sum(numbers: &[i32]) -> i32 {
    numbers.iter().sum()
}
```

而不是：

```rust
fn sum(numbers: Vec<i32>) -> i32 {
    numbers.iter().sum()
}
```

### 可以接受的场景

#### `Arc::clone`

```rust
let state = Arc::clone(&shared_state);
```

这是增加引用计数，不是深拷贝整个对象。

#### 小对象 clone

```rust
let id = user_id.clone();
```

如果类型很小，成本明确，可以接受。

#### 确实需要两份独立数据

```rust
let backup = config.clone();
```

### 结论

`clone()` 的本质是：

```rust
// 我不想处理 ownership，复制一份算了
```

不是不能用，但每一个 `clone()` 都应该有理由。

---

## 3. 函数参数 ownership 自检

### 优先级

如果函数只是读取数据，优先使用：

```rust
&T
&str
&[T]
```

如果函数需要修改数据，使用：

```rust
&mut T
```

如果函数需要保存、转移、发送到线程或异步任务中，才使用：

```rust
T
String
Vec<T>
```

> ⚠️ Agent 注意：把已有函数签名从 `String`/`Vec<T>` 改成 `&str`/`&[T]` 是**改动调用方契约**，不是局部优化。改之前先 `rg` 出所有调用点，确认能一次改干净；如果该函数用于 trait 方法、`async` 跨任务、或需要 `'static`，引用反而会引入生命周期问题（见第 9 条），此时**不要改**。新写的函数才优先用引用；存量签名的改动需先确认。

### 自检问题

1. 这个函数是否真的需要拥有参数？
2. 调用后，调用方是否还需要继续使用这个值？
3. 是否因为参数类型设计不合理，导致调用方大量 `clone()`？
4. `String` 是否可以改成 `&str`？
5. `Vec<T>` 是否可以改成 `&[T]`？

### 不推荐

```rust
fn log_message(message: String) {
    println!("{message}");
}
```

### 推荐

```rust
fn log_message(message: &str) {
    println!("{message}");
}
```

### 不推荐

```rust
fn process_items(items: Vec<Item>) {
    for item in items {
        println!("{:?}", item);
    }
}
```

### 推荐

```rust
fn process_items(items: &[Item]) {
    for item in items {
        println!("{:?}", item);
    }
}
```

---

## 4. `Option<T>` 自检

### 警惕写法

```rust
let value = None;
```

这通常会导致类型推断不清晰。

### 推荐写法

```rust
let value: Option<String> = None;
```

或者：

```rust
let value = None::<String>;
```

### 自检问题

1. `None` 的具体类型是否清楚？
2. `Option<T>` 是不是被用来隐藏错误原因？
3. 如果失败需要说明原因，是否应该用 `Result<T, E>`？
4. 是否存在连续多层 `Option<Option<T>>`？

### 不推荐

```rust
fn find_user(id: UserId) -> Option<User> {
    // 数据库错误和用户不存在都返回 None
}
```

### 推荐

```rust
fn find_user(id: UserId) -> Result<Option<User>, DbError> {
    // Ok(Some(user)) => 找到用户
    // Ok(None) => 用户不存在
    // Err(error) => 查询失败
}
```

### 结论

`Option<T>` 表示“可能没有”。

如果你需要表达“为什么没有”，用 `Result<T, E>`。

---

## 5. `Result<T, E>` 自检

### 警惕写法

```rust
fn do_work() -> Result<(), Box<dyn std::error::Error>> {
    // ...
}
```

这不是绝对错误，但在业务代码里经常太宽泛。

### 自检问题

1. 错误类型是否足够明确？
2. 调用方能否根据错误类型做不同处理？
3. 是否所有错误都被塞进了 `String` 或 `Box<dyn Error>`？
4. 是否应该定义自己的错误枚举？

### 推荐写法

```rust
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("user not found")]
    UserNotFound,

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("invalid input: {0}")]
    InvalidInput(String),
}
```

然后：

```rust
fn get_user(id: UserId) -> Result<User, AppError> {
    // ...
}
```

### 结论

错误类型也是 API 的一部分。

不要只为了省事就把所有错误都变成 `String`、`anyhow::Error` 或 `Box<dyn Error>`。

> ⚠️ Agent 注意：引入 `thiserror` / 自定义错误枚举来替换已有错误类型，会改变函数返回类型，波及所有 `?`、`map_err`、`match` 调用点，是牵一发动全身的改动（第 0 条第 3 项）。不要为了一个小修改顺手重构整套错误类型；需要时先列计划、经确认再做。

---

## 6. `String` / `&str` 自检

### 基本规则

```rust
&str    // 只读字符串，优先用于函数参数
String  // 需要拥有、存储、修改字符串
```

### 不推荐

```rust
fn validate_email(email: String) -> bool {
    email.contains('@')
}
```

### 推荐

```rust
fn validate_email(email: &str) -> bool {
    email.contains('@')
}
```

> ⚠️ Agent 注意：这条只适用于**新写的函数**。批量把存量函数的 `String` 参数改成 `&str` 属于第 0 条第 3 项的高风险改动，需先确认调用方并征得同意，不要为了“更规范”而成片改签名。

### 什么时候用 `String`

#### 结构体需要拥有数据

```rust
struct User {
    name: String,
}
```

#### 函数要返回新字符串

```rust
fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}
```

#### 需要修改字符串

```rust
let mut name = String::from("Alice");
name.push_str(" Smith");
```

### 自检问题

1. 函数参数中的 `String` 是否只是读取？
2. 是否导致调用方不得不 `.to_string()` 或 `.clone()`？
3. 返回值是否需要 ownership？
4. 结构体字段是否需要拥有数据？

---

## 7. `Vec<T>` / `&[T]` 自检

### 基本规则

```rust
&[T]    // 只读一组元素
&mut [T] // 修改已有元素
Vec<T>  // 拥有、扩容、返回新的集合
```

### 不推荐

```rust
fn print_items(items: Vec<Item>) {
    for item in items {
        println!("{:?}", item);
    }
}
```

### 推荐

```rust
fn print_items(items: &[Item]) {
    for item in items {
        println!("{:?}", item);
    }
}
```

> ⚠️ Agent 注意：同上，改存量 `Vec<T>` 参数为 `&[T]` 是改契约，先 `rg` 调用方再决定；新函数才默认用 slice。

### 自检问题

1. 函数是否真的需要拥有整个 `Vec`？
2. 是否只是遍历读取？
3. 是否需要支持数组、slice、Vec 等多种输入？
4. 是否造成调用方不必要的 clone？

---

## 8. 生命周期标注自检

### 警惕写法

```rust
fn get_name<'a>(user: &'a User) -> &'a str {
    &user.name
}
```

这可以写得更简单：

```rust
fn get_name(user: &User) -> &str {
    &user.name
}
```

### 自检问题

1. 生命周期标注是否真的必要？
2. 是否只是 AI 为了显得“更 Rust”乱加？
3. 编译器是否可以自动推断？
4. 返回引用是否来自多个输入之一？
5. 是否可以通过调整 ownership 避免复杂生命周期？

### 需要生命周期的典型场景

```rust
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() > b.len() {
        a
    } else {
        b
    }
}
```

因为返回值可能来自 `a`，也可能来自 `b`。

### 结论

生命周期不是越多越安全。

能省略就省略，除非返回引用和多个输入之间的关系需要明确表达。

---

## 9. `async` / `tokio::spawn` 自检

### 警惕写法

```rust
async fn process(data: &str) {
    tokio::spawn(async {
        println!("{data}");
    });
}
```

这类代码经常有生命周期问题。

### 推荐写法

```rust
async fn process(data: String) {
    tokio::spawn(async move {
        println!("{data}");
    });
}
```

### 自检问题

1. `tokio::spawn` 中捕获的变量是否满足 `'static`？
2. 是否错误地把引用传进后台任务？
3. 是否需要 `async move`？
4. 数据应该 clone、Arc，还是直接 move？
5. 是否把 `Arc<Mutex<_>>` 当成默认解法？

### 常见模式

#### 任务需要拥有数据

```rust
let data = data.clone();

tokio::spawn(async move {
    handle(data).await;
});
```

#### 共享只读配置

```rust
let config = Arc::clone(&config);

tokio::spawn(async move {
    use_config(config).await;
});
```

#### 共享可变状态

```rust
let state = Arc::clone(&state);

tokio::spawn(async move {
    let mut state = state.lock().await;
    state.count += 1;
});
```

### 结论

`async` 里的 ownership 要特别清楚。

不要为了让 `tokio::spawn` 编译通过，就盲目加 `clone()`、`Arc`、`Mutex`、`'static`。

---

## 10. `Arc<Mutex<T>>` 自检

### 警惕写法

```rust
let state = Arc::new(Mutex::new(State::new()));
```

这不是错误，但 AI 很容易过度使用。

### 自检问题

1. 真的需要多线程共享吗？
2. 真的需要可变共享吗？
3. 是否可以通过消息传递代替共享状态？
4. 是否可以把状态限制在单个 owner 中？
5. 是否会长时间持有锁？
6. 是否在 `.await` 期间持有锁？

### 不推荐

```rust
let mut state = state.lock().await;
do_async_work().await;
state.count += 1;
```

问题是：锁跨过了 `.await`。

### 推荐

```rust
let value = {
    let state = state.lock().await;
    state.value.clone()
};

do_async_work(value).await;
```

或者：

```rust
do_async_work().await;

let mut state = state.lock().await;
state.count += 1;
```

### 结论

`Arc<Mutex<T>>` 是共享可变状态的工具，不是默认架构。

能不用就不用，能缩小锁范围就缩小锁范围。

---

## 11. `Rc<RefCell<T>>` 自检

### 警惕写法

```rust
let state = Rc::new(RefCell::new(State::new()));
```

### 自检问题

1. 是否只是为了绕过 borrow checker？
2. 是否真的需要多个 owner？
3. 是否真的需要运行时可变借用检查？
4. 是否可以改成普通的 `&mut T`？
5. 是否可能发生重复可变借用导致 panic？

### 不推荐

```rust
fn update(state: Rc<RefCell<State>>) {
    state.borrow_mut().count += 1;
}
```

### 推荐

```rust
fn update(state: &mut State) {
    state.count += 1;
}
```

### 结论

`RefCell` 把借用检查从编译期推迟到运行时。

能用普通引用解决，就不要上 `Rc<RefCell<T>>`。

---

## 12. `Box<dyn Any>` / `serde_json::Value` 自检

### 警惕写法

```rust
Box<dyn Any>
```

```rust
serde_json::Value
```

它们很像 TypeScript 里的 `any`。

### 自检问题

1. 是否真的需要运行时动态类型？
2. 数据结构是否可以用 `enum` 表达？
3. JSON 是否可以反序列化成明确的 struct？
4. 下游代码是否到处在做类型判断？
5. 错误是否被推迟到了运行时？

### 不推荐

```rust
fn handle(value: serde_json::Value) {
    println!("{}", value["name"]);
}
```

### 推荐

```rust
#[derive(serde::Deserialize)]
struct User {
    name: String,
    age: u32,
}

fn handle(user: User) {
    println!("{}", user.name);
}
```

### 多种类型时，优先用 enum

```rust
enum Event {
    UserCreated { id: UserId, name: String },
    UserDeleted { id: UserId },
    PasswordChanged { id: UserId },
}
```

而不是：

```rust
struct Event {
    kind: String,
    payload: serde_json::Value,
}
```

### 结论

Rust 的优势是静态建模。

能用 `struct` / `enum` 表达，就不要长期依赖 `Any` 或 `Value`。

---

## 13. Trait 使用自检

### 常见形式

```rust
fn f<T: Trait>(x: T) {}
fn f(x: impl Trait) {}
fn f(x: &dyn Trait) {}
fn f(x: Box<dyn Trait>) {}
```

### 基本判断

#### 用泛型或 `impl Trait`

当类型在编译期已知，且不需要放进同一个集合：

```rust
fn handle<T: Handler>(handler: T) {
    handler.handle();
}
```

或者：

```rust
fn handle(handler: impl Handler) {
    handler.handle();
}
```

#### 用 `&dyn Trait`

当只需要借用 trait object：

```rust
fn handle(handler: &dyn Handler) {
    handler.handle();
}
```

#### 用 `Box<dyn Trait>`

当需要拥有一个动态分发对象：

```rust
struct App {
    handler: Box<dyn Handler>,
}
```

### 自检问题

1. 是否真的需要动态分发？
2. 是否只是 AI 随手用了 `Box<dyn Trait>`？
3. 是否可以用泛型？
4. 是否需要存储不同具体类型的对象？
5. trait 是否对象安全？

### 结论

不要默认使用 `Box<dyn Trait>`。

优先考虑泛型、`impl Trait`、引用，只有需要动态分发和 ownership 时再用 `Box<dyn Trait>`。

---

## 14. `unsafe` 自检

### 警惕写法

```rust
unsafe {
    // ...
}
```

### 自检问题

1. 这里是否真的必须 unsafe？
2. 是否有安全抽象可以替代？
3. unsafe 块是否尽可能小？
4. 是否写清楚了 safety invariant？
5. 是否封装在安全 API 后面？
6. 是否经过测试和审查？

### 推荐写法

```rust
// SAFETY:
// - ptr is non-null
// - ptr points to valid initialized memory
// - lifetime does not outlive the source buffer
unsafe {
    *ptr
}
```

### 结论

`unsafe` 不是“禁用 Rust 安全检查”那么简单。

用了 `unsafe`，你就接管了编译器原本帮你证明的事情。

---

## 15. 错误处理风格自检

### 不推荐

```rust
return Err("something went wrong".into());
```

```rust
Err(format!("failed: {}", e))
```

### 推荐

用明确错误类型：

```rust
#[derive(Debug, thiserror::Error)]
enum ServiceError {
    #[error("user not found: {0}")]
    UserNotFound(UserId),

    #[error("permission denied")]
    PermissionDenied,

    #[error("database error")]
    Database(#[from] sqlx::Error),
}
```

### 自检问题

1. 错误是否可分类？
2. 错误是否保留了原始 cause？
3. 错误信息是否对调用方有用？
4. 是否把所有错误都变成了字符串？
5. 是否应该用 `thiserror` 定义库/业务错误？
6. 是否应该用 `anyhow` 处理应用层错误？

### 粗略规则

```rust
thiserror // 适合库、业务层、需要明确错误类型
anyhow    // 适合应用入口、CLI、脚本、快速聚合错误
```

---

## 16. 模块和可见性自检

### 警惕写法

```rust
pub struct User {
    pub id: String,
    pub name: String,
    pub password_hash: String,
}
```

### 自检问题

1. 是否所有字段都应该公开？
2. 是否需要构造函数保证不变量？
3. 是否应该用 private 字段 + getter？
4. 模块边界是否清晰？
5. `pub` 是否被滥用？

### 推荐

```rust
pub struct User {
    id: UserId,
    name: String,
    password_hash: PasswordHash,
}

impl User {
    pub fn new(id: UserId, name: String, password_hash: PasswordHash) -> Self {
        Self {
            id,
            name,
            password_hash,
        }
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
```

### 结论

`pub` 是 API 承诺。

不要为了方便测试或调用，就把内部状态全部暴露出去。

> ⚠️ Agent 注意：把已有的 `pub` 字段改成 private + getter 会让所有直接访问该字段的代码编译失败，是连锁大改。这条用于**新设计**类型时；存量类型的收紧属于第 0 条第 3 项高风险改动，先 `rg` 出字段访问点并确认后再动。

---

## 17. 类型建模自检

### 警惕写法

```rust
type UserId = String;
type OrderId = String;
```

这样 `UserId` 和 `OrderId` 仍然可以互相误传。

### 推荐

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UserId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OrderId(String);
```

### 自检问题

1. 是否有多个语义不同但底层类型相同的值？
2. 是否容易把 `String`、`u64`、`Uuid` 传错？
3. 是否应该使用 newtype？
4. 是否可以用 enum 表达状态？
5. 是否使用 bool 表达了过多含义？

### 不推荐

```rust
fn set_user_status(user_id: String, active: bool) {
    // ...
}
```

### 推荐

```rust
struct UserId(String);

enum UserStatus {
    Active,
    Disabled,
    Pending,
}

fn set_user_status(user_id: UserId, status: UserStatus) {
    // ...
}
```

### 结论

好的 Rust 代码会把业务含义放进类型系统里，而不是靠注释和记忆。

---

## 18. bool 参数自检

### 警惕写法

```rust
create_user(name, true);
send_email(user, false);
```

调用方很难知道 `true` / `false` 代表什么。

### 推荐

使用 enum：

```rust
enum SendWelcomeEmail {
    Yes,
    No,
}

fn create_user(name: &str, send_email: SendWelcomeEmail) {
    // ...
}
```

或者拆成两个函数：

```rust
create_user(name);
create_user_with_welcome_email(name);
```

### 自检问题

1. bool 参数在调用处是否一眼能看懂？
2. 是否未来可能扩展到第三种状态？
3. 是否应该用 enum？
4. 是否应该拆函数？

---

## 19. Iterator 自检

### 警惕写法

```rust
let mut result = Vec::new();

for item in items {
    if item.active {
        result.push(item.name.clone());
    }
}
```

这不是错，但可以更清晰。

### 推荐

```rust
let result: Vec<_> = items
    .iter()
    .filter(|item| item.active)
    .map(|item| item.name.clone())
    .collect();
```

### 但也不要过度函数式

不推荐为了炫技写复杂链式调用：

```rust
let result = items
    .iter()
    .filter(...)
    .flat_map(...)
    .map(...)
    .filter(...)
    .fold(...);
```

如果可读性下降，普通 `for` 更好。

### 自检问题

1. `for` 循环是否更清楚？
2. iterator 链是否过长？
3. 是否为了避免 clone 可以返回引用？
4. 是否需要 `into_iter()` 消费集合？
5. 是否需要 `iter()` 借用集合？

---

## 20. `iter()` / `into_iter()` / `iter_mut()` 自检

### 基本区别

```rust
items.iter()      // 借用每个元素：&T
items.iter_mut()  // 可变借用每个元素：&mut T
items.into_iter() // 消费集合，拿到 T
```

### 自检问题

1. 是否真的要消费整个集合？
2. 后面是否还要继续使用原集合？
3. 是否只是读取？
4. 是否只是修改元素？
5. 是否因为误用 `into_iter()` 导致 move 问题？

### 示例

只读：

```rust
for item in items.iter() {
    println!("{:?}", item);
}
```

修改：

```rust
for item in items.iter_mut() {
    item.active = false;
}
```

消费：

```rust
for item in items.into_iter() {
    process(item);
}
```

---

## 21. `Default` 自检

### 警惕写法

```rust
let config = Config::default();
```

### 自检问题

1. 默认值是否业务上安全？
2. 是否会隐藏必要配置缺失？
3. 是否应该要求显式传参？
4. 默认值是否适合生产环境？
5. 是否应该使用 builder？

### 推荐

```rust
let config = ConfigBuilder::new()
    .host("localhost")
    .port(3000)
    .build()?;
```

### 结论

`Default` 很方便，但不要让它隐藏关键配置。

---

## 22. `derive` 自检

### 常见写法

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct User {
    id: UserId,
    name: String,
}
```

### 自检问题

1. `Clone` 是否真的需要？
2. `Debug` 是否会泄露敏感信息？
3. `Serialize` 是否会暴露不该输出的字段？
4. `PartialEq` 是否符合业务语义？
5. `Hash` 是否稳定可靠？

### 警惕

```rust
#[derive(Debug)]
struct User {
    email: String,
    password_hash: String,
    token: String,
}
```

日志里可能泄露敏感信息。

### 推荐

手写 `Debug`：

```rust
impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("email", &self.email)
            .field("password_hash", &"<redacted>")
            .field("token", &"<redacted>")
            .finish()
    }
}
```

---

## 23. 日志自检

### 警惕写法

```rust
println!("user: {:?}", user);
dbg!(user);
```

### 推荐

使用 tracing：

```rust
tracing::info!(user_id = %user.id(), "user created");
tracing::error!(error = %err, "failed to create user");
```

### 自检问题

1. 是否用了 `println!` 当日志？
2. 是否泄露 token、密码、密钥、cookie？
3. 是否包含足够上下文？
4. 是否有结构化字段？
5. 错误日志是否保留 error chain？

---

## 24. 测试自检

### 自检问题

1. 是否只测 happy path？
2. 是否测试错误分支？
3. 是否测试边界条件？
4. 是否测试空输入？
5. 是否测试非法输入？
6. 是否测试并发或异步行为？
7. 是否测试序列化/反序列化兼容性？

### 推荐结构

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_user_when_user_exists() {
        // arrange
        // act
        // assert
    }

    #[test]
    fn returns_error_when_user_missing() {
        // arrange
        // act
        // assert
    }
}
```

### 结论

Rust 类型系统能防很多问题，但不能替你验证业务逻辑。

---

## 25. Clippy 自检

建议至少跑：

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

更严格：

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### 常见 Clippy 会抓的问题

* 不必要的 clone
* 不必要的 collect
* 可以用 `if let` 简化的 match
* 可疑的字符串转换
* 不规范的 API 写法
* 不必要的 lifetime
* 低效的 iterator 使用

### 结论

AI 生成的 Rust 代码，至少要过一遍 `cargo fmt`、`cargo clippy`、`cargo test`。

---

## 26. 目录和模块组织自检

### 基本原则

目录不是越深越好，也不是所有代码都塞进 `lib.rs` / `main.rs`。

模块应该按职责拆分，而不是按“暂时方便”堆放。

### 推荐结构

```text
src/
  lib.rs
  error.rs
  config.rs
  domain/
    mod.rs
    user.rs
    order.rs
  service/
    mod.rs
    user_service.rs
  adapter/
    mod.rs
    db.rs
    http.rs
  cli/
    mod.rs
    args.rs
```

### 自检问题

1. `main.rs` 是否只负责启动、参数解析、依赖组装？
2. `lib.rs` 是否只负责导出稳定 API，而不是承载大量实现？
3. 业务模型、服务逻辑、外部适配器是否分层清楚？
4. 模块名是否表达职责，而不是 `utils`、`helpers`、`misc` 泛滥？
5. 测试是否靠近被测模块，还是散落到难以维护？

### 目录职责建议

```text
domain/   // 业务类型、值对象、领域规则
service/  // 业务流程、用例编排
adapter/  // 数据库、HTTP、文件系统、外部命令
infra/    // 运行时、日志、配置加载等基础设施
cli/      // 命令行参数、输出格式、交互入口
api/      // HTTP handler、DTO、请求响应边界
```

### 结论

目录结构应该让读代码的人快速知道：

```rust
// 业务规则在哪里？
// 外部系统在哪里接入？
// 程序入口在哪里组装依赖？
```

如果找一个功能需要在多个无语义的 `utils.rs` 里翻来翻去，说明目录结构需要重整。

---

## 27. 文件行数和拆分自检

### 规则

单个 Rust 源文件超过 1000 行时，应**评估**是否拆分（评估 = 判断，不等于一定要拆）。

超过 1000 行不是绝对错误，但不能默认接受。需要在 Review 中说明为什么暂时不能拆。

> ⚠️ Agent 注意：“评估拆分”不是“立即拆分”。拆分文件属于第 0 条第 3 项高风险改动：会移动 `mod` 边界、可能被迫把内部实现 `pub` 出去、影响所有 `use` 路径。除非用户明确要求拆分，否则只**报告**“此文件 N 行、职责混杂、建议这样拆”，由用户决定。绝不为了降行数做机械切割。

### 拆分优先级

1. 按职责拆：类型、错误、解析、执行、输出、测试分别放到独立模块。
2. 按边界拆：业务逻辑和外部 I/O 分开。
3. 按状态拆：状态定义、状态转移、事件处理、渲染/输出分开。
4. 按测试拆：大量测试可以放进 `tests.rs` 或 `tests/` 子模块。

### 不推荐

```text
src/provider.rs      // 3000 行，包含配置、鉴权、HTTP、解析、测试
src/runtime.rs       // 2000 行，包含状态机、工具调用、日志、重试
```

### 推荐

```text
src/provider/
  mod.rs
  auth.rs
  request.rs
  response.rs
  retry.rs
  tests.rs
```

### 自检问题

1. 文件是否超过 1000 行？
2. 是否存在多个互不相关的职责？
3. 是否因为文件过大导致 Review 很难定位变化？
4. 是否有大段测试、常量、fixture 可以移出？
5. 拆分后 public API 是否仍然收敛，而不是把内部实现全 `pub` 出去？

### 结论

大文件最常见的问题不是“行数多”，而是职责边界已经模糊。

超过 1000 行时，优先拆出稳定职责；不要只为了降行数做机械切割。

---

## 28. 公共单元和重复逻辑自检

### 警惕写法

```rust
fn parse_user_id(raw: &str) -> Result<UserId, Error> {
    // ...
}

fn parse_owner_id(raw: &str) -> Result<UserId, Error> {
    // 几乎一样
}
```

### 自检问题

1. 同一段业务规则是否在多个模块重复实现？
2. 同一个格式化、校验、解析、重试、路径处理是否复制了多份？
3. 公共逻辑是否应该放到领域类型、trait、helper 函数或小服务里？
4. 抽象是否真的减少重复，还是只是制造间接层？
5. 公共单元是否有单元测试覆盖核心分支？

### 推荐方向

```rust
impl UserId {
    pub fn parse(raw: &str) -> Result<Self, UserIdParseError> {
        // 所有 UserId 解析规则集中在这里
    }
}
```

或者：

```rust
fn retry_policy_for(provider: ProviderKind) -> RetryPolicy {
    // 重试规则集中维护
}
```

### 结论

“可公用单元公用”不是把所有东西丢进 `utils.rs`。

公共单元应该有清楚语义、明确输入输出、稳定测试，并且放在最接近业务含义的位置。

---

## 29. 局部变量和 magic number 自检

### 警惕写法

```rust
let a = 30;
let b = 1024 * 1024;
let c = input.trim().to_lowercase().replace('-', "_");

if retries > 3 && elapsed_ms > 5000 {
    // ...
}
```

### 自检问题

1. 局部变量是否只是转手，没有提升可读性？
2. 临时变量名是否表达了业务含义？
3. 数字、字符串、超时时间、重试次数是否是 magic number？
4. 相同常量是否在多个位置重复？
5. 是否应该用 newtype、enum、配置项或常量表达？

### 推荐

```rust
const MAX_RETRY_ATTEMPTS: u32 = 3;
const REQUEST_TIMEOUT_MS: u64 = 5_000;
const BYTES_PER_MIB: usize = 1024 * 1024;
```

```rust
let normalized_name = input.trim().to_lowercase().replace('-', "_");
```

### 注意

不是所有数字都要变成常量。

```rust
for index in 0..items.len() {
    // 这里的 0 不需要抽象
}
```

### 结论

减少局部变量，不是把表达式写得越长越好。

目标是让变量承担语义，让常量承担规则，让读者不用猜 `3`、`5000`、`"active"` 到底代表什么。

---

## 30. 工具、外部调用和适配器抽象自检

### 警惕写法

```rust
Command::new("git").arg("status").output()?;
reqwest::get(url).await?;
std::fs::read_to_string(path)?;
```

这些代码直接散落在业务逻辑里，会让测试、重试、错误处理和权限控制变难。

### 自检问题

1. 外部命令、HTTP、文件系统、数据库调用是否散落在业务流程中？
2. 是否需要统一超时、重试、日志、指标、权限检查？
3. 是否可以通过 trait 或 adapter 封装，以便测试替换？
4. 错误是否在边界处转换成领域错误？
5. 工具调用参数是否经过结构化建模，而不是拼字符串？

### 推荐

```rust
trait GitClient {
    fn status(&self, repo: &RepoPath) -> Result<GitStatus, GitError>;
}

struct SystemGitClient;

impl GitClient for SystemGitClient {
    fn status(&self, repo: &RepoPath) -> Result<GitStatus, GitError> {
        // 只在 adapter 里调用 Command
    }
}
```

### 结论

业务代码应该表达“我要做什么”，适配器代码才表达“怎么调用外部工具”。

工具抽象化的目的不是炫技，而是统一边界、便于测试、减少重复的命令拼接和错误处理。

---

## 31. 状态机和重复参数自检

### 警惕写法

```rust
fn transition(
    session_id: SessionId,
    user_id: UserId,
    current_state: State,
    next_state: State,
    retry_count: u32,
    timeout_ms: u64,
    event: Event,
) -> Result<State, Error> {
    // ...
}
```

参数越来越多时，状态边界通常已经不清楚。

### 自检问题

1. 状态机是否把同一组参数在多个函数间重复传递？
2. 是否应该把共享上下文收敛成 `Context` / `StateMachine` / `TransitionInput`？
3. 状态转移是否只由明确事件触发？
4. 是否存在非法状态可以被构造出来？
5. 是否用 `bool` 或字符串表达状态，而不是 enum？
6. 状态进入、退出、副作用是否混在一个巨大函数里？

### 推荐

```rust
struct TransitionContext {
    session_id: SessionId,
    user_id: UserId,
    policy: RetryPolicy,
}

struct StateMachine {
    context: TransitionContext,
    state: State,
}

impl StateMachine {
    fn apply(&mut self, event: Event) -> Result<(), TransitionError> {
        self.state = self.state.transition(event, &self.context)?;
        Ok(())
    }
}
```

### 结论

状态机不要靠一长串重复参数维持上下文。

把稳定上下文放进结构体，把变化输入建成事件，把合法状态转移放进类型系统和测试里。

---

## 32. 死代码和死逻辑自检

### 警惕写法

```rust
if false {
    legacy_path();
}
```

```rust
match state {
    State::Ready => run(),
    State::Ready => retry(),
    _ => {}
}
```

```rust
fn old_parser() {
    // 没有任何调用方
}
```

### 自检问题

1. 是否存在永远不会进入的分支？
2. 是否存在无调用方的旧函数、旧类型、旧模块？
3. 是否存在已经被新逻辑覆盖的 fallback？
4. 是否存在保留但不再使用的配置项、feature flag、环境变量？
5. 删除后是否有测试能证明行为没有丢失？

### 推荐处理

```text
删除死代码前（缺一不可）：
1. 用 rg 确认无调用方——并把搜索范围扩到整个 workspace，注意宏、pub 导出、cfg(feature)、跨 crate 调用、运行时注册等隐藏引用。
2. 用测试确认旧分支没有承担隐藏行为。
3. 删除关联测试、fixture、配置和文档中的过期引用。
4. 如果暂时不能删、或判不准是否在用，写清楚情况并交回用户决定，不要擅自删。
```

### 结论

死代码会让 Review 误判系统真实行为。

> ⚠️ Agent 注意：删除属于第 0 条第 3 项高风险且**不可逆**的改动。agent 判断“无调用方”经常漏看（宏、导出、feature、反射式注册）。只有在上面 4 步全部确认、且删除在当前任务范围内时才删；**判不准就报告，不要删**。不要为了“以后可能用”保留确认无用的旧逻辑，但“可能在用”比“可能没用”更值得保守。

---

## 33. 写死特殊规则和伪泛化分支自检

Agent 常见问题之一，是为了让当前样例、当前测试、当前 prompt 通过，写出看似“智能”、实际只匹配特例的逻辑。

### 警惕写法

```rust
if input.contains("Alice") {
    return Ok(UserRole::Admin);
}
```

```rust
match provider_name.as_str() {
    "openai" => handle_openai(),
    "anthropic" => handle_anthropic(),
    "test-provider-from-fixture" => Ok(Default::default()),
    _ => handle_default(),
}
```

```rust
let rules = HashMap::from([
    ("error one", "fixed output one"),
    ("error two", "fixed output two"),
    ("benchmark case 3", "fixed output three"),
]);

if let Some(output) = rules.get(input) {
    return Ok(output.to_string());
}
```

### 自检问题

1. 是否为了某个测试样例、fixture、用户名、文件名、provider 名称写了特殊分支？
2. 这条规则是否来自真实业务协议，还是 Agent 猜出来的？
3. 字典 / map 是否只是把输入样例映射到输出样例，而没有真实算法？
4. `match` 分支是否每个都有明确业务含义，还是为了覆盖 prompt 里的几个词？
5. fallback 是否掩盖了本该报错的未知输入？
6. 新增一个相似输入时，逻辑是否会立刻失效？
7. 是否应该改成解析器、配置、策略表、trait 实现或数据驱动规则？

### 不推荐

```rust
fn classify_error(message: &str) -> ErrorKind {
    if message.contains("timeout in benchmark") {
        ErrorKind::Retryable
    } else if message.contains("fixture missing") {
        ErrorKind::Ignored
    } else {
        ErrorKind::Unknown
    }
}
```

### 推荐

```rust
enum ErrorKind {
    Retryable,
    Permanent,
    InvalidInput,
}

fn classify_error(error: &ServiceError) -> ErrorKind {
    match error {
        ServiceError::Timeout(_) => ErrorKind::Retryable,
        ServiceError::Validation(_) => ErrorKind::InvalidInput,
        ServiceError::Auth(_) => ErrorKind::Permanent,
    }
}
```

### 字典匹配可以接受的场景

字典不是不能用，但必须有真实语义：

```rust
static MIME_TYPES: &[(&str, &str)] = &[
    ("json", "application/json"),
    ("html", "text/html"),
    ("png", "image/png"),
];
```

可以接受：

* 标准协议映射
* 明确配置表
* 稳定枚举到展示文案
* provider / tool / command 的真实注册表

不可接受：

* 把测试输入硬编码到 map
* 把 benchmark 样例硬编码成输出
* 通过 `contains()` 猜测业务状态
* 用 `_ => default` 吞掉未知分支

### 结论

写死特殊规则是 Agent 最危险的“看似能跑”坏味道之一。

如果规则没有来源、没有类型、没有测试覆盖泛化输入，就应该重写成真实业务逻辑，而不是继续补丁式加分支。

---

# 高频坏味道速查表

| 坏味道                 | 可能问题              | 优先替代                              |
| ------------------- | ----------------- | --------------------------------- |
| `unwrap()`          | 逃避错误处理            | `?` / `ok_or` / `match`           |
| 到处 `clone()`        | 逃避 ownership 设计   | `&T` / `&str` / `&[T]`            |
| `Box<dyn Any>`      | 类似 TS any         | `enum` / trait / 泛型               |
| `serde_json::Value` | 类型信息丢失            | struct / enum + serde             |
| `Arc<Mutex<T>>`     | 共享可变状态过度          | 单 owner / message passing / 缩小锁范围 |
| `Rc<RefCell<T>>`    | 绕过 borrow checker | `&mut T` / 重构 ownership           |
| 乱加 `'static`        | 生命周期设计不清          | 明确 ownership / move               |
| 复杂 lifetime         | API 设计可能过复杂       | 让编译器推断 / 调整 ownership             |
| `String` 参数         | 不必要 ownership     | `&str`                            |
| `Vec<T>` 参数         | 不必要 ownership     | `&[T]`                            |
| `bool` 参数           | 调用语义不清            | enum / 拆函数                        |
| `pub` 到处开           | API 边界失控          | private 字段 + 方法                   |
| `String` 当 ID       | 类型语义弱             | newtype                           |
| `println!` 日志       | 不适合生产             | `tracing` / `log`                 |
| `unsafe`            | 手动承担安全证明          | 安全抽象 / 缩小 unsafe 块                |
| 文件超过 1000 行        | 职责边界模糊            | 按职责拆模块 / 移出测试和适配器               |
| 到处 `utils`          | 公共逻辑无语义           | 按领域或边界命名公共单元                    |
| magic number        | 规则不可读、难维护         | 命名常量 / 配置 / newtype              |
| 外部调用散落业务代码        | 难测试、难统一错误和权限      | adapter / trait / client 抽象       |
| 状态机重复传一串参数        | 上下文边界不清           | `Context` / `StateMachine` / event |
| 死代码死分支             | 行为噪音、误导 Review     | 删除旧逻辑 / 清理配置和测试引用                |
| 写死特殊规则             | 只过当前样例，不是真逻辑      | 真实解析 / 类型建模 / 策略抽象                |
| 无意义字典匹配            | 输入输出样例硬编码          | 算法、协议表、配置表或注册表                   |
| `contains()` 判业务     | 模糊匹配导致误判           | 结构化字段 / enum / parser            |
| `_ => default` 吞未知分支 | 错误被隐藏              | 显式错误 / unknown variant 处理          |

---

# AI / Agent 生成 Rust 代码审查重点

## 第一优先级

先搜这些关键词：

```text
unwrap
expect
clone
Arc<Mutex
Rc<RefCell
serde_json::Value
Box<dyn Any
unsafe
'static
```

这些不一定错，但每一个都值得人工看一眼。

---

## 第二优先级

检查函数签名：

```rust
fn foo(x: String)
fn foo(xs: Vec<T>)
fn foo(user: User)
```

问：

1. 是否只是读取？
2. 是否可以改成 `&str`、`&[T]`、`&User`？
3. 是否因为签名设计不好导致调用方 clone？

---

## 第三优先级

检查错误处理：

```rust
unwrap()
expect()
map_err(|e| e.to_string())
Result<T, String>
Box<dyn Error>
```

问：

1. 错误是否应该被调用方处理？
2. 是否需要具体错误类型？
3. 是否隐藏了错误原因？
4. 是否导致服务直接 panic？

---

## 第四优先级

检查状态共享：

```rust
Arc<Mutex<T>>
Rc<RefCell<T>>
static mut
lazy_static
OnceCell
```

问：

1. 真的需要全局状态吗？
2. 真的需要共享可变状态吗？
3. 是否可以通过参数传递？
4. 是否可以通过 channel/message passing？
5. 锁是否跨 `.await`？

---

## 第五优先级

检查结构和重复：

```text
超过 1000 行的 .rs 文件
utils.rs / helper.rs / misc.rs
重复 parse / validate / normalize / retry 逻辑
magic number / magic string
直接 Command / reqwest / fs / db 调用
状态机重复参数
无调用方函数和死分支
写死 fixture / benchmark / provider 特例
无意义 HashMap 输入输出匹配
contains() / starts_with() 猜业务分支
_ => default 吞未知输入
```

问：

1. 文件是否需要按职责拆分？
2. 公共逻辑是否应该收敛到有语义的单元？
3. 外部工具调用是否应该放到 adapter？
4. 状态机上下文是否应该结构化？
5. 旧代码是否可以删除？
6. 特殊分支是否来自真实业务规则？
7. 字典匹配是否只是硬编码样例？

---

# 最小审查流程

每次审查 Rust 代码，可以按这个顺序：

## 1. 先跑工具

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## 2. 搜高危关键词

```bash
rg "unwrap|expect|clone|Arc<Mutex|Rc<RefCell|serde_json::Value|Box<dyn Any|unsafe|'static"
```

## 3. 看函数签名

重点检查：

```rust
String
Vec<T>
HashMap<K, V>
User
Config
```

这些是否被不必要地按值传递。

## 4. 看错误边界

确认：

* 库代码不要随便 panic
* 业务代码不要吞错误
* API 错误类型要清楚
* `Option` 和 `Result` 不要混用错语义

## 5. 看并发状态

确认：

* 不要无脑 `Arc<Mutex<_>>`
* 不要锁跨 `.await`
* 不要共享本可以局部拥有的数据

## 6. 看结构和死代码

确认：

* 单文件超过 1000 行时必须评估拆分
* 公共逻辑不要复制多份
* magic number 要变成命名常量或配置
* 外部工具、HTTP、文件系统、数据库调用要有边界抽象
* 状态机不要重复传递同一组上下文参数
* 死代码、死分支、过期配置及时删除
* 写死的特殊规则必须有真实来源
* 字典匹配不能只是把样例输入映射到样例输出
* `contains()`、`starts_with()` 不能替代结构化解析

---

# 简短判断原则

## `unwrap()`

问：

> 这里失败是“不可能”，还是“我懒得处理”？

如果是后者，改掉。

## `clone()`

问：

> 这里是真的需要两份数据，还是函数参数应该改成引用？

如果是后者，改掉。

## `Arc<Mutex<T>>`

问：

> 这里是真的需要多任务共享可变状态，还是我没设计 ownership？

如果是后者，改掉。

## `serde_json::Value`

问：

> 这里的数据结构真的未知，还是我懒得定义类型？

如果是后者，改成 struct / enum。

## 生命周期

问：

> 这里的生命周期关系真的复杂，还是我乱加标注？

如果是后者，删掉，让编译器推断。

## 大文件

问：

> 这个文件超过 1000 行，是因为一个职责真的复杂，还是多个职责混在一起？

如果是后者，拆模块。

## magic number

问：

> 这个数字或字符串是普通语法需要，还是业务规则？

如果是业务规则，命名、配置化或建模。

## 状态机

问：

> 这些参数是每次事件都不同，还是稳定上下文被重复传来传去？

如果是后者，收敛到上下文结构体或状态机对象。

## 死代码

问：

> 这段逻辑还有真实调用方和真实业务意义吗？

如果没有，删掉。

## 特殊规则

问：

> 这个分支是业务规则，还是为了当前样例刚好通过？

如果是后者，重写成真实逻辑。

## 字典匹配

问：

> 这个 map 是协议/配置/注册表，还是测试输入到测试输出的硬编码？

如果是后者，删除并实现真正的解析或算法。

---

# 总结

好的 Rust 代码通常有这些特征：

* 参数尽量借用，而不是随便拿 ownership
* 错误路径清楚，不靠 `unwrap()` 硬崩
* 类型表达业务含义，而不是到处 `String` / `Value`
* clone 有明确理由，不是为了糊过 borrow checker
* 并发状态边界清楚，不滥用 `Arc<Mutex<_>>`
* 生命周期标注少而必要
* `unsafe` 极少，并且有明确 safety 说明
* API 边界清晰，`pub` 不滥用
* 目录按职责组织，大文件超过 1000 行会主动拆分
* 公共规则集中维护，不在各处复制粘贴
* magic number、magic string 有命名或建模
* 外部工具调用通过 adapter / client 收敛
* 状态机上下文清晰，不靠重复参数传递
* 死代码、死分支、过期配置及时删除
* 特殊规则有真实来源，不为当前样例硬编码
* 字典和分支表达协议或配置，不伪装成算法
* 通过 `fmt`、`clippy`、`test`

一句话：

> Rust 不是让你用 `clone()` 和 `unwrap()` 把代码写到能编译，而是逼你把 ownership、error handling、state sharing 这些设计问题讲清楚。
