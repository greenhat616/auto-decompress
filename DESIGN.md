# auto-decompress 设计文档

## 1. 项目目标

自动化还原经过**多次压缩**、**后缀篡改**、**容器伪装**（伪装为图片/视频等）、**分卷拆散**的压缩文件，递归解压至最终内容。

核心场景：
- 文件被反复压缩（`.tar.gz` → `.7z` → `.rar`）
- 后缀被篡改或插入非 ASCII 字符（`.rar.删去` → `.rar`）
- 压缩文件被塞入图片/视频容器伪装（`photo.jpg` 实际为 `.zip`）
- 分卷文件被散落在不同目录、不同深度、甚至重命名

## 2. 整体架构

```
┌─────────────────────────────────────────────────────┐
│                   auto-decompress                    │
│                   (orchestrator)                     │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Input Paths ──► Scanner ──► Task Queue ──► Loop    │
│                                    ▲           │    │
│                                    └───────────┘    │
│                                                     │
│  Loop 内部（单文件处理流水线）：                       │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐        │
│  │ Normalize │──►│ Identify │──►│ Extract  │        │
│  │ Filename  │   │ FileType │   │ Archive  │        │
│  └──────────┘   └──────────┘   └──────────┘        │
│       │              │              │               │
│       │              │              ▼               │
│       │              │     ┌──────────────┐         │
│       │              │     │ 输出文件重新   │         │
│       │              │     │ 入队扫描       │         │
│       │              │     └──────────────┘         │
│       │              │                              │
│  ┌────┴──────────────┴──────────────────────┐       │
│  │  Password Manager (内嵌密码表 + 尝试)      │       │
│  └──────────────────────────────────────────┘       │
│                                                     │
│  ┌──────────────────────────────────────────┐       │
│  │  Volume Assembler (分卷识别与重组)         │       │
│  └──────────────────────────────────────────┘       │
│                                                     │
├─────────────────────────────────────────────────────┤
│  normalize-filename   │  bit7z-rs                   │
│  (文件名标准化 +       │  (7-zip FFI 解压引擎)       │
│   文件类型识别)         │                             │
└─────────────────────────────────────────────────────┘
```

## 3. Workspace Crate 规划

```
auto-decompress/
├── Cargo.toml                    # workspace root
├── normalize-filename/           # [已有] 文件名标准化 + 文件类型检测
├── bit7z-rs/                     # [已有] 7-zip C++ FFI 绑定
├── auto-decompress-core/         # [新建] 核心调度引擎
│   ├── src/
│   │   ├── lib.rs
│   │   ├── scanner.rs            # 输入路径扫描，展开为文件列表
│   │   ├── task_queue.rs         # 任务队列（按深度排序、去重、优先级）
│   │   ├── pipeline.rs           # 单文件处理流水线
│   │   ├── volume_assembler.rs   # 分卷识别与重组
│   │   ├── password_manager.rs   # 密码表管理与尝试策略
│   │   ├── container_probe.rs    # 容器内嵌检测
│   │   └── config.rs             # 运行时配置
│   └── Cargo.toml
└── auto-decompress-cli/          # [新建] CLI 入口
    ├── src/
    │   └── main.rs
    └── Cargo.toml
```

## 4. 核心流程

### 4.1 主循环

```
fn run(input_paths: Vec<PathBuf>, config: Config) -> Result<Report>

1. Scanner: 递归展开 input_paths → Vec<FileEntry>
   - 每个 FileEntry 记录：绝对路径、相对路径、深度
   - 按路径深度升序排序（浅层优先）

2. VolumeAssembler: 扫描文件列表，识别分卷组
   - 将分卷组聚合为 VolumeGroup
   - 非分卷文件保持为单独条目

3. TaskQueue: 初始化队列
   - 单文件 → SingleFileTask
   - 分卷组 → VolumeGroupTask

4. Loop: 逐任务处理，直到队列为空
   for task in queue.drain() {
       match process(task) {
           Extracted(new_files) => {
               // 新文件经过 Normalize + Identify
               // 如果仍是压缩文件 → 重新入队
               queue.push_batch(new_files);
           }
           NotArchive => { /* 记录为最终文件 */ }
           Failed(err) => { /* 记录错误，继续 */ }
       }
   }

5. Report: 输出解压报告
```

### 4.2 单文件处理流水线 (Pipeline)

```
fn process(entry: FileEntry) -> PipelineResult

Step 1: Normalize Filename
  - 删除非 ASCII 字符：".rar.删去" → ".rar"
  - 标准化组合后缀：".t删去gz" → ".tgz"
  - 去重复后缀：".zip.zip" → ".zip"
  - 输出：标准化后的文件名（可能重命名文件）

Step 2: Identify File Type
  - Magic number 检测（file_type crate，优先级最高）
  - 扩展名交叉验证
  - 若 magic ≠ extension → 以 magic 为准，记录 warning
  - 容器内嵌探测（见 4.5）
  - 输出：DetectedType { format, confidence, is_archive }

Step 3: Extract (if is_archive)
  - 通过 bit7z-rs 打开
  - 若加密 → PasswordManager 逐密码尝试
  - 若分卷 → 使用 open_multi_volume
  - 解压到临时目录
  - 输出文件列表返回主循环
```

### 4.3 分卷识别与重组 (VolumeAssembler)

分卷文件的核心难点：可能被散布到不同目录，甚至被重命名。

#### 4.3.1 分卷命名模式库

```rust
/// 已知的分卷命名模式
enum VolumePattern {
    // RAR 风格
    RarClassic,       // .rar, .r00, .r01, ...
    RarNew,           // .part1.rar, .part2.rar, ...

    // ZIP 风格
    ZipSplit,         // .zip, .z01, .z02, ...

    // 7z 风格
    SevenZipSplit,    // .7z.001, .7z.002, ...

    // 通用编号
    GenericNumbered,  // .001, .002, .003, ...

    // 自定义（正则匹配）
    Custom(Regex),
}
```

每种模式对应的正则：
```
RAR classic:  ^(.+)\.(rar|r\d{2,3})$
RAR new:      ^(.+)\.part(\d+)\.rar$
ZIP split:    ^(.+)\.(zip|z\d{2,3})$
7z split:     ^(.+\.7z)\.(\d{3,})$
Generic:      ^(.+)\.(\d{3,})$
```

#### 4.3.2 跨目录分卷发现算法

```
fn assemble_volumes(files: &[FileEntry]) -> Vec<VolumeGroup>

Phase 1: 本地聚合（同目录）
  - 按目录分组
  - 在每组内按模式匹配分卷
  - 大多数正常情况在此完成

Phase 2: 跨目录模糊匹配（处理分卷被分散的情况）
  - 对 Phase 1 中不完整的分卷组（缺少连续编号）
  - 提取分卷基名（stem），构建搜索键
  - 搜索键包含：
    a. 精确基名匹配
    b. 模糊基名匹配（Levenshtein 距离 ≤ 2）
    c. 文件大小一致性（分卷大小通常相等，末卷可较小）
    d. magic number 一致性
  - 在全部文件中搜索匹配候选
  - 评分排序：
    Score = name_similarity * 0.4
          + size_consistency * 0.3
          + magic_match * 0.2
          + depth_proximity * 0.1

Phase 3: 验证
  - 验证分卷编号连续性（允许从 0 或 1 开始）
  - 使用 bit7z-rs 尝试打开分卷组（最终验证）
  - 打开失败 → 降级为单文件处理
```

#### 4.3.3 分卷排序与首卷识别

```rust
struct VolumeGroup {
    stem: String,
    pattern: VolumePattern,
    /// 按卷号排序的分卷路径
    volumes: Vec<(u32, PathBuf)>,
    /// 首卷路径（传给 bit7z-rs::open_multi_volume）
    first_volume: PathBuf,
}
```

首卷识别规则：
| 格式 | 首卷标识 |
|------|---------|
| RAR classic | `.rar` 文件（非 `.r00`） |
| RAR new | `.part1.rar` |
| ZIP split | `.zip` 文件（非 `.z01`） |
| 7z split | `.7z.001` |
| Generic | `.001` |

### 4.4 密码管理 (PasswordManager)

```rust
struct PasswordManager {
    /// 内嵌常见密码表
    builtin_passwords: Vec<String>,
    /// 用户自定义密码
    custom_passwords: Vec<String>,
    /// 从文件名/路径中提取的候选密码
    contextual_passwords: Vec<String>,
    /// 已成功使用过的密码（优先尝试）
    known_good: Vec<String>,
}
```

#### 4.4.1 密码尝试策略

```
fn try_passwords(archive: &Path) -> Option<String>

优先级：
1. known_good（历史成功密码，同一批任务中复用率高）
2. contextual_passwords（从文件名、父目录名提取）
3. custom_passwords（用户通过配置/CLI 提供）
4. builtin_passwords（常见密码表）

快速失败：
- 若 archive header 加密 → 打开时即可判断密码是否正确
- 若仅 content 加密 → 需要尝试解压一个小文件
- 设定最大尝试次数（默认 100），超出则跳过并报告
```

#### 4.4.2 上下文密码提取

从文件环境中推断可能的密码：
```
来源                           示例
────────────────────────────────────────
文件名中括号/方括号内的文本    "archive[password123].rar" → "password123"
同目录下的 .txt/.nfo 文件     读取内容，匹配 "密码"/"password"/"pass" 后的值
父目录名                      "解压密码是abc/" → "abc"
文件名中的 URL 片段           "www.example.com" → 作为密码候选
```

#### 4.4.3 内嵌密码表

```rust
const BUILTIN_PASSWORDS: &[&str] = &[
    "",                    // 空密码（最先尝试）
    "123456", "password", "123456789", "12345678",
    "1234", "qwerty", "abc123", "111111",
    // 中文互联网常见
    "解压密码", "www.example.com",
    // ... 可扩展
];
```

### 4.5 容器内嵌检测 (ContainerProbe)

某些文件将压缩包数据追加到合法容器文件（图片、视频）末尾。

#### 4.5.1 检测策略

```
fn probe_embedded(path: &Path) -> Option<EmbeddedArchive>

策略 1: 尾部 magic 扫描
  - 读取文件尾部 N 字节（默认 1MB）
  - 反向搜索已知压缩格式的 magic number：
    PK\x03\x04 (ZIP)
    Rar!\x1A\x07 (RAR)
    7z\xBC\xAF\x27\x1C (7z)
  - 若找到 → 记录偏移量

策略 2: 容器格式解析
  - JPEG: 在 FFD9 (EOI marker) 之后查找附加数据
  - PNG: 在 IEND chunk 之后查找附加数据
  - GIF: 在 trailer (0x3B) 之后查找
  - MP4/MOV: 解析 atom/box 结构，检查是否有未声明的尾部数据
  - AVI: 解析 RIFF 结构，检查尾部溢出

策略 3: 大小异常检测
  - 比较文件大小与容器声明大小
  - 若 actual_size >> declared_size → 可能有嵌入数据

检测到嵌入后：
  - 从偏移量处截取数据到临时文件
  - 对截取的数据运行正常的 Identify + Extract 流水线
```

#### 4.5.2 已知的嵌入技巧

| 技巧 | 原理 | 检测方法 |
|------|------|---------|
| JPEG + ZIP | ZIP 在 JPEG EOI 后追加 | 搜索 FFD9 后的 PK 签名 |
| PNG + RAR | RAR 在 IEND 后追加 | 解析 PNG chunks，检查 IEND 后数据 |
| BMP + 7z | 7z 在 BMP 像素数据后追加 | BMP header 声明大小 vs 实际大小 |
| MP4 + ZIP | ZIP 在 moov/mdat 后追加 | MP4 atom 解析，检查总长度 |
| GIF + RAR | 数据在 GIF trailer 后 | 搜索 0x3B 后的 Rar! 签名 |

### 4.6 文件类型检测增强

当前 `file_type` crate 基于 magic number，对某些场景不足：

```
检测层级（由快到慢，由粗到精）：

Level 0: 扩展名（清洗后）
  - 最快，但不可信
  - 仅作为 hint

Level 1: Magic number（file_type crate）
  - 读取前 N 字节
  - 大部分格式可识别
  - 对 split volume 第 2+ 卷可能失败

Level 2: 深度探测
  - 对 Level 1 无法识别的文件
  - 尝试用 bit7z-rs 以各种格式打开
  - 尝试容器内嵌探测
  - 用于分卷中间卷（无 magic header）

Level 3: Magika（可选）
  - Google 的 ML 文件类型识别模型
  - 对混淆文件的补充
  - 作为 optional feature
```

## 5. 关键数据结构

```rust
/// 扫描到的文件条目
struct FileEntry {
    path: Utf8PathBuf,
    depth: u32,
    size: u64,
    /// 标准化后的文件名
    normalized_name: String,
    /// 检测到的文件类型
    detected_type: Option<DetectedType>,
}

/// 文件类型检测结果
struct DetectedType {
    /// 主类型（如 "ZIP Format", "RAR Archive"）
    name: String,
    /// 对应的 bit7z ArchiveFormat（如果是压缩文件）
    archive_format: Option<ArchiveFormat>,
    /// 检测置信度
    confidence: Confidence,
    /// 是否为容器内嵌
    embedded: Option<EmbeddedInfo>,
}

enum Confidence {
    High,    // magic number 精确匹配
    Medium,  // 推断（如扩展名+大小模式）
    Low,     // 仅扩展名或 ML 模型猜测
}

struct EmbeddedInfo {
    /// 容器格式
    container_format: String,
    /// 嵌入数据的起始偏移
    offset: u64,
    /// 嵌入数据的长度（如果已知）
    length: Option<u64>,
}

/// 任务队列中的任务
enum Task {
    SingleFile(FileEntry),
    VolumeGroup(VolumeGroup),
}

/// 处理结果
enum ProcessResult {
    /// 解压成功，产出新文件
    Extracted {
        source: Task,
        output_files: Vec<FileEntry>,
        password_used: Option<String>,
    },
    /// 非压缩文件
    NotArchive(FileEntry),
    /// 处理失败
    Failed {
        source: Task,
        error: Error,
    },
}

/// 运行配置
struct Config {
    /// 最大递归深度（防止 zip bomb）
    max_recursion_depth: u32,
    /// 最大解压总大小（防止 zip bomb）
    max_total_size: u64,
    /// 自定义密码列表
    passwords: Vec<String>,
    /// 密码最大尝试次数
    max_password_attempts: u32,
    /// 7z.dll 路径
    library_path: PathBuf,
    /// 输出目录
    output_dir: PathBuf,
    /// 是否启用容器内嵌检测
    enable_container_probe: bool,
    /// 是否启用跨目录分卷搜索
    enable_cross_dir_volume_search: bool,
    /// 分卷模糊匹配阈值（Levenshtein 距离）
    volume_fuzzy_threshold: u32,
}
```

## 6. 安全防护

### 6.1 Zip Bomb 防护

```
防护措施：
1. max_recursion_depth（默认 10）—— 限制递归层数
2. max_total_size（默认 10GB）—— 限制总解压大小
3. 压缩比检测 —— 若 uncompressed/compressed > 100，发出警告
4. 单文件大小限制 —— 解压过程中监控输出大小
5. 递归计数器 —— 同一文件被反复解压超过阈值则终止
```

### 6.2 路径穿越防护

```
所有解压输出路径必须：
1. 解析为绝对路径后，仍在 output_dir 之下
2. 不包含 ".." 路径组件
3. 不为符号链接（或符号链接目标仍在 output_dir 下）
```

### 6.3 资源限制

```
- 并发解压任务数限制
- 单次密码尝试间隔（防止 CPU 占满）
- 临时文件清理（每轮完成后清理无用临时文件）
```

## 7. CLI 接口设计

```
auto-decompress [OPTIONS] <PATHS>...

ARGS:
    <PATHS>...              输入路径（目录或文件）

OPTIONS:
    -o, --output <DIR>      输出目录 [默认: ./output]
    -p, --password <PWD>    添加密码（可多次指定）
    --password-file <FILE>  密码列表文件（每行一个）
    --max-depth <N>         最大递归深度 [默认: 10]
    --max-size <SIZE>       最大解压总大小 [默认: 10G]
    --no-container-probe    禁用容器内嵌检测
    --no-cross-dir-volume   禁用跨目录分卷搜索
    --library <PATH>        7z.dll/7z.so 路径
    --dry-run               仅扫描，不解压
    -v, --verbose           详细输出
    -q, --quiet             安静模式
```

## 8. 错误处理策略

核心原则：**尽力而为，绝不中断**。单个文件处理失败不影响其他文件。

```rust
enum ProcessError {
    /// 文件类型无法识别
    UnrecognizedType { path: PathBuf },
    /// 所有密码均失败
    PasswordExhausted { path: PathBuf, attempts: u32 },
    /// 分卷不完整
    IncompleteVolume { group: VolumeGroup, missing: Vec<u32> },
    /// 解压失败
    ExtractionFailed { path: PathBuf, reason: String },
    /// 安全限制触发
    SecurityLimit { kind: SecurityLimitKind },
    /// IO 错误
    Io(std::io::Error),
}

enum SecurityLimitKind {
    MaxRecursionDepth,
    MaxTotalSize,
    SuspiciousCompressionRatio,
    PathTraversal,
}
```

所有错误收集到最终报告中，不提前终止。

## 9. 输出报告

```
auto-decompress Report
══════════════════════════════════════
Input: 3 paths, 47 files scanned
Extracted: 32 archives, 156 files output
Failed: 2 archives
Duration: 12.3s

Failures:
  ✗ data/encrypted.rar — 密码穷举失败 (100 次尝试)
  ✗ data/broken.7z.002 — 分卷不完整 (缺少 .001)

Warnings:
  ⚠ photos/image.jpg — 检测到容器内嵌 ZIP (偏移 0x1A3F)
  ⚠ docs/file.rar.删去 — 文件名已标准化为 file.rar

Volume Groups Assembled:
  ● backup.part[1-5].rar — 5 卷, 跨 2 个目录
```

## 10. 待讨论的开放问题

### Q1: 分卷文件的深度不同时，以哪个深度入队？

**建议**：以首卷深度为准。首卷通常是"主文件"，其他卷是附属。

### Q2: 分卷被重命名到完全不同的名字怎么办？

**建议**：当 Phase 2 模糊匹配无法找到候选时，回退到 Phase 3 暴力策略——对所有大小相近、magic 类似的"孤儿文件"进行两两组合尝试 `open_multi_volume`。代价高昂，仅在配置启用且文件数量可控时执行。

### Q3: 容器内嵌检测的误报如何处理？

**建议**：分级处理。若尾部数据能被成功打开为合法压缩文件 → 高置信度，执行解压；若仅 magic 匹配但打开失败 → 记录 warning，不解压。

### Q4: 密码提取的隐私/安全考量？

**建议**：上下文密码提取仅在本地执行，不上传任何数据。密码表不落盘（仅运行时内存中），自定义密码文件由用户自行管理。

### Q5: 递归解压的终止条件？

终止条件（满足任一即停）：
1. 队列为空
2. 达到 max_recursion_depth
3. 达到 max_total_size
4. 同一文件被解压超过 3 次（内容不变，可能是误识别）
5. 用户中断 (Ctrl+C graceful shutdown)
