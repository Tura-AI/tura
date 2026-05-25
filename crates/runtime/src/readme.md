session_management:{
    session_id: 16位16进制id(并非永久id而是runtime id)
    session_名称:自然语言名称
    session目录位置： 系统绝对目录位置
    session是否使用docker:
    session_topic:session的任务总类分类
    session当前轮次: 包括整个树状向下的session轮次的总和：
    session_log:[]（session的历史执行context log数组）
    session_created_at: session的创建时间
    session_last_update_at:session的上次激活时间（session的上次激活时间）
    session_started_at: 本次session的开启时间
    input:{user_input：用户开启任务的原始输入文本，
           file_input:[{file名称和目录，
                        file的大小，
                        file的最后修改时间，
                        file的描述备注，
                        }]
            }
    user_goal: 提炼总结的的用户总命令
    task_plan:
        [{
        step_task:子任务的描述，
        step_turn：包括该子任务的子进程和所有相应进程的轮次次数，
        step_memory:召回的完成这个子任务所需记忆文本，
        step_tool:召回的子任务所需工具json文本，
        step_context:子任务所需的上下文文本，
        step_agent_name:该子任务的执行agent，
        step_deliverable描述:子任务的交付目标描述
        step_deliverable目录:子任务的交付目标的绝对路径位置
        }]
    }






agent_management:{
    agent_id: 16位16进制id(并非永久id而是runtime id)
    agent_名称:自然语言名称
    agent目录位置： agent绝对目录位置    
    parent_agent_id: 下发任务的上游agent id (并非永久id而是runtime id，如果直接向用户报告则为空)
    report_to_user: 是否直接向用户报告
    provider:{tura_llm_name:模型使用的llm配置，
                stream:
                temperature:
                max_tokens:
                tool_choice: auto, strict, disable
                time_out_ms:
    }
    agent_prompt:[{
    agent_prompt:自然语言名称
    agent目录位置： 系统绝对目录位置
    }]
    agent_capabilities:[{
    capability_name:自然语言名称
    capability目录位置： 系统绝对目录位置
    }]
    validator:{
    need_validator:agent交付任务是否需要经过validator
    validator_name:agent的validator的名字
    }
}



runtime_management:{
    runtime_id:这次llm call 的16位16进制id
    created_at：runtime创建的时间戳
    called_at:runtime执行消费的时间戳
    first_token_at: 收到第一个llm回调的时间
    call_finshed_at:完整llm回调结束的时间
    call_result_status:llm方的回调 enum
    fallback_from_id:如果这次runtime是之前失败的fallback 则为他的id
    session_id: 对应的session id（为直接session 不是母session）
    agent_id: 16位16进制id(并非永久id而是runtime id，为直接agent不是母agent)
    provider:{
        tura_llm_name:模型使用的llm配置，
        stream:
        temperature:
        max_tokens:
        tool_choice: auto, strict, disable
        time_out_ms:
        thinking:
        provider_name:
        model_name:
        provider_url_name:
        provider_router_name:使用的tura_llm内部的providerrouter的名称        
        }
    error:{
        error_code:模型error code
        error_text:模型失败的回调描述
        retry_allowed:
        fullback_allowed:
        full_back_to_id:
        }
    reasoning:
    reasoning_hash:
    text:
    tool_call:[{
        tool_called_name:工具执行的名称
        tool_called_input:{}(工具执行的调用参数的json)
        tool_received_at:完整收到一个工具的toolcall的时间
        tool_executed_at: toolcall执行消费的时间
        tool_calldate_received_at:收到工具执行的tool calldata的时间
        tool_reported_success:单词工具调用本身，工具是否返回成功
        }，
        agent_reported_sucess: agent自己是否觉得执行成功（只能针对一次call的所有工具统一评价）
        agent_reported_helpful: agent自己觉得本地调用的结果是否能帮助执行任务（只能针对一次call的所有工具统一评价）
        agent_reported_sumamry: agent自己对本地工具调用的成功与否的评价（只能针对一次call的所有工具统一评价）
        validator_reported_sucess：validator对整个子任务评估后认为整体这次任务失败还是成功
        }]
    usage:
        {input_tokens
        output_tokens
        total_tokens
        cached_input_tokens
        cache_write_tokens
        reasoning_tokens
        attachement_input_tokens
        input_cost
        output_cost
        total_cost
        currency
        pricing_source
        latency_ms
        time_to_first_token_ms
        token_per_second
        }
    
}


在mano 下
├─ mano/mod.rs
├─ mano/process.rs
├─ manas/mod.rs
├─ manas/process.rs
4个文件，mano为一个 agent 系统的入口，参考 state_machine下的3个状态机文件，

mano/mod.rs 是一个抽象session入口+可外部调用服务+声明层， mano/process.rs 为内部执行session逻辑，两者是整个系统的核心入口。

manas/mod.rs 是agent的抽象入口+prompt+记忆+上下文+工具的装载, manas/process.rs 为内部核心的agent配置逻辑，两者是在mano后的模块。
mano是针对user的入口，manas是针对agent的入口。 mano和manas都需要一个服务入口，占用2个不同的端口

mano有两个入口一个是 process_from_user 另一个是process_from_agent
上游会传入用户意图和上下文，如果有附件的话还有附件的目录位置传入（session_management中的input:{user_input：用户开启任务的原始输入文本，
file_input:[{file名称和目录，
file的大小，
file的最后修改时间，
file的描述备注，
}]
}）
这些信息进入mano入口后，mano会把参数透传到 mano/process.rs，process首先去调用 session.rs，session.rs是一个入口文件他会调用 /session目录下的代码文件:
activate_session：这个文件会创建一个空的session_management对象，只先临时使用目前项目的系统目录下的/test_session作为session目录位置，把创建好的session对象调用同一/session目录下的create_session，
create_session:这个文件会拿到session_management状态机。 生成session id，用当前的系统时间创造一个临时的session name，使用上游传入的shession目录, session 默认不使用docker，session_topic：默认为general， session当前轮次为0， session log 留空， session_created_at为 世界标准时间， session_last_update_at和session_started_at也为当前标准时间。input为上游透传的input参数。 user_goal直接使用input的user_input。创建一个空的task_plan对象。
然后返回整个session状态机给 activate_session，activate_session再返回对象给 mano/process.rs

mano/process.rs 拿到这个对象后调用同一目录下的 manas，manas再调用 manas/process.rs，process调用 agent_router脚本，agent_router.rs脚本也是一个入口文件，他会调用 /agent_router目录下的代码文件：
activate_agent:这个文件会用agent_management状态机预设统一的 coding_agent 参数
agent_id 随机生成，
名称为对应的名称，
目录位置先用项目目录的名称
parent_agent_id为空
report_to_user 为 true
provider都使用tura_coder，stream 为yes，tempature 为0.5 max token 为空， tool choice 为 auto，time_out_ms 为 90秒
agent_prompt 为 coding_agent，
agent_prompt目录为 项目目录下 crates/agents/src/{agent_name}，由 manas/agent_prompts.rs 按顺序读取 persona.md、communication_style.md、prompt.md；agent配置从 crates/agents/src/{agent_name}/agent_config.json 读取
agent_capabilities 使用统一 coding_agent 能力集；当前版本只保留 command_run，其中 command_run 只支持 shell/bash 控制台命令和 apply_patch.
（不需要找对应的tools 先都mock）
validator_name 为空， need_validator 为 false，

activate_agent会返回激活的对象的一个数组的agent_management状态机给 manas/process.rs。

让mana 和manas的入口非常简洁，用户可以引用这2个文件然后override他们 不经过 agent_router和session直接硬编码或者其他路径传入对应参数。
