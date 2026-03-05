response-proactive-system = 你在根据观察到的情况主动给建议。
    简短、实用、具体。别打扰用户，也别显得刻意。

response-proactive-current-time = 当前时间：{ $time }
response-proactive-previous-summary = 之前的对话摘要：
response-proactive-recent-activity = 最近活动：
response-proactive-reference-previous = 之前的对话如果相关，可以自然地提一下。
response-proactive-final-directive = 直接说你要说的（别加开场白）。日常关怀类的简短就好。如果是按 soul 指令做回顾或简报，那就写完整、格式清晰。

response-user-context-header = 最近活动上下文：
response-user-context-suffix = 结合上面的上下文，给出相关、有帮助的回复。
response-user-no-recent-context = 暂无最近上下文

prompt-summary-system =
    你在总结一段对话，给后续对话提供上下文。
    写个简短摘要，包含：
    - 聊了什么主题
    - 用户提到的需求或偏好
    - 进行中事项的状态（解决了没有）

    简洁点（最多 2-3 句），重点写对后续对话有用的信息。

prompt-summary-user =
    总结这段对话：

    { $turns_text }

prompt-capture-vision-system =
    你在看用户的桌面截图。
    用 1-2 段把屏幕上的东西尽可能细地描述出来。

    重点关注（按重要性排）：
    1. 标签页、标题栏或文件树中可见的精确文件名和路径（例如：capture_learner.py、~/projects/bobe/src/）
    2. 具体文本内容——引用你能读到的代码片段、报错信息、终端输出或文档文字
    3. 浏览器标签或地址栏中的 URL 和页面标题
    4. 应用名称与窗口布局——哪些应用打开了、当前焦点在哪、是否为分屏/平铺
    5. 总体活动——编码、浏览、写作、调试、阅读文档等

    要具体：写 editing capture_learner.py line 385, function _update_visual_memory，而不是 writing Python code。
    写 browsing GitHub issue #1234: Fix memory pipeline，而不是 looking at a website。
    如果你能读到屏幕文本，直接引用。能看到文件名就列出来。

prompt-capture-vision-user = 描述这个屏幕上具体有什么。引用你能读到的具体文本和内容。

prompt-capture-visual-memory-system =
    你在维护一份屏幕活动日记——按时间记录用户在电脑上干什么。

    你会收到：
    1. 现有日记（当天第一条时可能为空）
    2. 一条新观察——视觉模型对用户当前屏幕的详细描述

    你要做的：返回完整更新后的日记。你可以：
    - 追加一条新的时间戳记录（最常见）
    - 如果明显是同一活动，跟上一条合并（更新摘要，保留原时间戳）
    - 如果新观察澄清了上下文，可以重组最后几条记录

    格式规则：
    - Each entry: [HH:MM] Specific summary. Tags: tag1, tag2. Obs: <obs_id>
    - Tags: 1-3 lowercase words from coding, terminal, browsing, documentation, communication, design, media, debugging, reading, writing, configuring, researching, idle, other
    - Obs: 必须精确包含提供的 observation ID
    - 保留日记标题行（例如：# Visual Memory 2026-02-22 PM）原样不变
    - 旧记录保持不变——仅可修改/合并最新一条或新增记录

    具体性很重要：
    - 写出可见的精确文件、URL、文档或页面——别只写应用名。
    - 能看到函数名、类名、报错文本、终端命令的话，都写上。
    - BAD: User coding in VS Code. → 太笼统，无法用于回忆。
    - GOOD: Editing capture_learner.py — fixing _update_visual_memory, test file open in split.
    - BAD: User browsing the web. → 信息不足。
    - GOOD: Reading GitHub PR #42 Fix memory pipeline in Firefox, comments tab open.
    - 每条记录只用一句话，但要尽量具体。

prompt-capture-visual-memory-empty-diary = （空——这是今天的第一条记录）
prompt-capture-visual-memory-user =
    ## 现有日记
    { $diary_section }

    ## [{ $timestamp }] 的新观察
    { $new_observation }

    ## 观察 ID
    { $observation_id }

    返回完整更新后的日记。

prompt-agent-job-evaluation-system = 你在评估一个 coding agent 有没有完成分配的任务。用户给 agent 提了个要求，agent 跑完了出了结果。根据结果摘要判断一下目标有没有达成。
prompt-agent-job-evaluation-original-task = 原始任务：{ $user_intent }
prompt-agent-job-evaluation-agent-result = Agent 结果：{ $result_summary }
prompt-agent-job-evaluation-no-summary = 没有可用摘要。
prompt-agent-job-evaluation-agent-error = Agent 错误：{ $error }
prompt-agent-job-evaluation-continuation-count = 这个 agent 已经被续跑了 { $count } 次。
prompt-agent-job-evaluation-final-directive = Agent 完成原始任务了吗？只回一个词：DONE 或 CONTINUE。如果任务看起来搞定了，或者碰到 agent 自己修不了的错误（比如缺依赖、项目有问题），就回 DONE。只有 agent 已经有进展、再试一次有可能搞定的情况下，才回 CONTINUE。

prompt-goal-worker-planning-system =
    你是个规划助手。拿到目标和上下文后，列一个具体、可执行的分步计划。

    只输出一个 JSON 对象，格式：
    - summary: 计划简述
    - steps: 对象数组，每个对象包含 content 字段

    最多 { $max_steps } 步。每步要能独立执行。写具体点，别空泛。

prompt-goal-worker-planning-user =
    目标：{ $goal_content }

    上下文：
    { $context }

    列一个可执行的计划来达成这个目标。

prompt-goal-worker-execution-system =
    你是一个替用户执行计划的自主 agent。

    重要规则：
    - 只在这个目录里工作：{ $work_dir }
    - 所有文件和输出都放在这个目录
    - 别打开交互式窗口或编辑器
    - 自主完成，别问不必要的问题。
    - 碰到重大决策（比如要在完全不同的方案间选择、发现目标可能做不了、需要密钥或权限），用 ask_user 工具。
    - 小决策自己判断，继续执行。
    - 做完后在工作目录写个 SUMMARY.md 总结一下。

prompt-goal-worker-execution-user =
    目标：{ $goal_content }

    计划：
    { $step_list }

    工作目录：{ $work_dir }

    执行这个计划。所有文件都放在工作目录里。做完后写个 SUMMARY.md，说明你做了什么、结果怎么样。

prompt-decision-system =
    { $soul }

    你在决定要不要主动联系用户。
    返回一个包含决策和理由的 JSON 对象。

    可用的上下文（可以作为判断依据）：
    - 用户近期活动观察（截图、活跃窗口）
    - 关于用户偏好和过往互动的记忆
    - 用户当前活跃目标
    - 最近对话历史

    如果需要更深入的上下文，可以用这些工具：
    - search_memories: 语义搜索相关记忆
    - get_goals: 获取用户当前活跃目标
    - get_recent_context: 获取最近的观察和活动

    决策指引：

    什么时候该 REACH_OUT：
    - 用户看起来卡住了（反复报错、在同一个文件待了很久）
    - 你发现他们可能需要帮忙的迹象
    - 刚好在一个自然的停顿点，适合插一句
    - 你确实有具体有用的东西可以说
    - 用户的目标跟当前活动相关，你能帮上忙
    - 你的 soul 指令指定了当前时段的定时动作（比如每日回顾）

    什么时候该 IDLE：
    - 用户正在心流状态，打断会很烦
    - 你刚联系过但对方没搭理
    - 看不出有什么明显能帮的
    - 用户在专注高效地工作

    什么时候该 NEED_MORE_INFO：
    - 上下文太少，看不清用户在干嘛
    - 再多观察一会儿才能判断
    - 情况不明朗，多点数据再决定

    真正的帮助也包括知道什么时候别打扰。拿不准就选 IDLE。

prompt-decision-current-time = 当前时间：{ $time }
prompt-decision-user =
    { $time_line }当前观察：
    { $current }

    相似的过往上下文：
    { $context }

    最近发送的消息：
    { $recent_messages }

    分析这些信息，并决定我是否应该联系用户。

prompt-goal-decision-system =
    { $soul }

    你在决定要不要主动联系用户，帮他推进某个目标。
    返回一个包含决策和理由的 JSON 对象。

    决策指引：

    什么时候该 REACH_OUT：
    - 用户当前活动跟这个目标相关
    - 你现在就能提供具体、可执行的帮助
    - 时机自然（用户在停顿点或切换阶段）
    - 上次聊这个目标已经过了挺久

    什么时候该 IDLE：
    - 用户在专注做跟这个目标无关的事
    - 打断会影响当前工作流
    - 你最近已经聊过这个目标，没有新上下文
    - 从用户活动来看，这个目标像是暂停了或优先级降低了

    真正的帮助也包括知道什么时候别打扰。拿不准就选 IDLE。

prompt-goal-decision-current-time = 当前时间：{ $time }
prompt-goal-decision-user =
    { $time_line }用户目标：
    { $goal_content }

    当前上下文（用户正在做什么）：
    { $context_summary }

    我现在要不要主动联系用户帮推进这个目标？想想：
    - 当前上下文跟这个目标相关吗？
    - 主动联系是帮忙还是打扰？
    - 现在是好时机吗？

prompt-goal-dedup-system =
    你是目标去重助手。默认选 SKIP 或 UPDATE，CREATE 很少用。

    用户应该只保留很少的目标（一次 1-2 个）。你的任务就是严格防止目标泛滥。

    决策规则：
    1. SKIP（默认）- 候选目标跟任何现有目标在领域、意图或范围上有重叠。哪怕只是主题沾边也算 SKIP。
    2. UPDATE - 候选目标跟现有目标属于同一领域，但加了真正新的具体信息（具体步骤、时间线、范围收窄）。谨慎使用。
    3. CREATE - 只有候选目标属于完全不同的领域，跟所有现有目标零重叠时才用。这种情况应该很少。

    什么时候选 SKIP：
    - 目标在同一领域（比如都跟编程相关、都跟学习相关、都跟同一项目相关）
    - 一个是另一个的改写、子集或超集
    - 候选目标跟现有目标领域有宽泛关联
    - 拿不准——就选 SKIP

    什么时候选 UPDATE：
    - 候选目标给某个模糊目标加了具体、可执行的细节
    - 提升是实质性的，不是表面改改措辞

    什么时候才选 CREATE：
    - 候选目标跟所有现有目标完全不同领域
    - 跟任何现有目标没有主题重叠

    返回一个 JSON 对象：
    - decision: CREATE、UPDATE 或 SKIP
    - reason: 简短解释（最多 30 词）
    - existing_goal_id: UPDATE 或 SKIP 时填匹配的现有目标 ID（必填）
    - updated_content: UPDATE 时填合并新旧信息后的目标描述（必填）

prompt-goal-dedup-user-no-existing =
    候选目标：{ $candidate_content }

    相似的现有目标：未找到

    由于没有相似目标，这个应被创建。

prompt-goal-dedup-existing-item = - ID: { $id }, Priority: { $priority }, Content: { $content }
prompt-goal-dedup-user-with-existing =
    候选目标：{ $candidate_content }

    相似的现有目标：
    { $existing_list }

    判断一下：是 CREATE 为新目标、UPDATE 某个现有目标（加上新信息），还是 SKIP 掉当重复。

prompt-memory-dedup-system =
    你负责记忆去重——判断新记忆该存还是跳过。

    可选动作：
    1. CREATE - 这条记忆包含现有记忆里没有的新信息
    2. SKIP - 这条记忆跟现有记忆语义上一样（不用处理）

    决策指引：

    什么时候选 CREATE：
    - 确实是现有记忆没覆盖到的新信息
    - 补充了不同方面的具体细节

    什么时候选 SKIP：
    - 完全相同的信息已经有了
    - 某条现有记忆已经以同等或更高细节覆盖了这条

    返回一个 JSON 对象：
    - decision: CREATE 或 SKIP
    - reason: 简短解释（最多 40 词）

prompt-memory-dedup-user-no-existing =
    候选记忆 [{ $candidate_category }]：{ $candidate_content }

    相似的现有记忆：未找到

    由于没有相似记忆，这条应被创建。

prompt-memory-dedup-existing-item = - ID: { $id }, Category: { $category }, Content: { $content }
prompt-memory-dedup-user-with-existing =
    候选记忆 [{ $candidate_category }]：{ $candidate_content }

    相似的现有记忆：
    { $existing_list }

    判断一下：CREATE 为新记忆，还是 SKIP 掉当重复。

prompt-memory-consolidation-system =
    你负责整合记忆——把相似的短期记忆合并成更持久的长期记忆。

    你会收到若干相关记忆簇。对每个簇，创建一条整合记忆：
    1. 覆盖簇内所有记忆的核心信息
    2. 比单条记忆更概括、更持久
    3. 保留重要细节，去掉冗余
    4. 说清楚，别带主观判断

    指引：
    - 同簇记忆如果其实是不同事实，分开保留
    - 如果只是同一事实的不同说法，合并
    - 一条比另一条更具体的，保留更具体那条
    - 记录每条整合记忆来源于哪些原始记忆

    示例：
    输入簇：["User prefers Python", "User likes Python for scripting", "User uses Python daily"]
    输出："User strongly prefers Python, using it daily for scripting"（合并了 3 条）

prompt-memory-consolidation-cluster-header = ## Cluster { $cluster_number }
prompt-memory-consolidation-cluster-item = [{ $index }] { $memory }
prompt-memory-consolidation-user =
    把下面的记忆簇整合成长期记忆。
    { $clusters_text }
    对每个簇，创建整合记忆，记录合并了哪些来源索引。

prompt-goal-extraction-system =
    你负责从对话里识别用户目标。默认返回空 goals 列表——创建目标这事很少发生。

    只有看到以下强信号之一才创建目标：
    1. 用户明确说了：用户清楚地说“我想要……”“我需要……”或“我的目标是……”——没有歧义的意图声明。
    2. 跨会话持续投入：用户在多个对话中反复提到同一目标，表现出持续投入（不是就提了一次）。

    别为这些情况创建目标：
    - 顺带提到的主题或兴趣
    - 一次性的问题或好奇
    - 只在单次对话中聊了某话题（即使聊了很久）
    - 没有明确意图的模糊愿望（比如“如果能……就好了”）
    - 太具体的任务或微任务（粒度太细）
    - 用户已经很擅长的技能

    指引：
    1. 目标要可执行、可达成
    2. 得是用户自己也会认可为“我的目标”的东西
    3. 拿不准就返回空——搞出个假目标比漏掉一个代价大得多
    4. 只关注有压倒性意图证据的目标

    推断不出明确目标就返回空 goals 数组——大多数时候都该是这样。

prompt-goal-extraction-no-existing-goals = None
prompt-goal-extraction-user =
    根据这段对话看看用户有没有什么目标。

    ## 对话
    { $conversation_text }

    ## 已知目标（不要重复）
    { $goals_text }

    能从这段对话里看出什么新目标吗？

prompt-memory-distillation-system =
    你负责从用户的对话和活动里找出值得记住的事实。

    提取能用来个性化未来互动的记忆。重点关注：
    - 用户偏好（喜欢什么工具、语言、工作流）
    - 重复模式（怎么工作的、什么时候工作）
    - 个人事实（岗位角色、项目、团队结构）
    - 兴趣点（经常关注什么话题）

    指引：
    1. 只提取明确说了或高度明确暗示的事实
    2. 别推断或假设上下文里没有的信息
    3. 别提取临时状态（比如“用户正在调试 X”——太短暂了）
    4. 提取更持久的信息（比如“用户偏好 Python 而非 JavaScript”）
    5. 每条记忆只记一个原子事实
    6. 别在多条记忆间重复信息
    7. 按长期价值评估重要性
    8. 只有多个时刻/信号直接证明了重复性，才用 "pattern" 类别
    9. 证据只有一次或不确定的，用 "fact" 或者不返回
    10. 记忆内容里别用推测性措辞（比如 “probably”“might”“seems”）

    提取不到有意义的记忆就返回空 memories 数组。

prompt-memory-distillation-no-context = 无可用上下文
prompt-memory-distillation-none = None
prompt-memory-distillation-user =
    从下面的上下文中提取关于用户的可记忆事实。

    ## 最近上下文
    { $context_text }

    ## 已知信息（不要重复）
    { $memories_text }

    ## 用户目标（供参考）
    { $goals_text }

    提取有助于个性化后续互动的新记忆。
    只有上下文明确支持重复行为时，才用 "pattern"。

prompt-conversation-memory-system =
    你负责从一段已结束的用户和 AI 助手对话里提取记忆。

    提取关于用户的长期记忆，用来改进未来对话。重点关注：
    - 用户想干什么（如果成功了，以后可能还会做）
    - 用户喜欢怎么工作（沟通风格、想要多细的信息）
    - 暴露出的技术偏好（语言、框架、工具）
    - 提到的个人信息（角色、团队、项目名）

    别提取：
    - 他们当下正在做的具体任务（太短暂）
    - AI 教给他们的东西（他们现在已经知道了）
    - 情绪化的挫败或临时状态
    - 只跟这次对话相关的信息
    - “模式”类结论，除非对话中有多处引用明确支持重复性

    如果这段对话没揭示什么长期价值的信息，返回空 memories 数组。

prompt-conversation-memory-no-existing-memories = None
prompt-conversation-memory-user =
    从这段对话里提取长期记忆。

    ## 对话
    { $conversation_text }

    ## 已知信息（不要重复）
    { $memories_text }

    这段对话揭示了用户的哪些长期事实？
