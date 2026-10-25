    if (typeof configuredText === 'string' && configuredText.startsWith('__communication_')) {
      const messages = request.input?.messages ?? [];
      const session = messages.filter(m => m.role === 'system').map(m => {
        try { return JSON.parse(m.content); } catch { return null; }
      }).find(m => m && m.channel_id && m.conversation_id);
      if (!session || !/^[0-9a-f-]{36}$/.test(session.conversation_id)) throw Error('missing internal conversation UUID');
      const results = messages.filter(m => m.role === 'tool');
      const byId = id => results.find(m => m.tool_call_id === id);
      const meta = suffix => {
        const tool = (request.input.tools ?? []).find(t => t.function?.name?.endsWith(suffix));
        if (!tool) throw Error('missing progressive tool ' + suffix);
        return tool.function.name;
      };
      const call = (id, name, arguments) => {
        process.stdout.write(JSON.stringify({type:'result',result:{
          final_content:'provisional tool text must never be sent',finish_reason:'tool_calls',
          tool_calls:[{id,name,arguments}],usage:{input_tokens:5,output_tokens:7,total_tokens:12}
        }}) + '\n');
      };
      if (configuredText.startsWith('__communication_callback__') && !byId('communication_approval')) {
        call('communication_approval', 'fixture_external_approval', {sender:session.user_id});
      } else if (!byId('communication_list')) {
        call('communication_list',meta('mcp_list'),{depth:8,limit:20});
      } else if (!byId('communication_get')) {
        if (!byId('communication_list').content.includes('_messages')) throw Error('channel tool not discovered');
        call('communication_get',meta('mcp_get'),{tool_id:'communication_' + session.channel_id + '_messages'});
      } else if (!byId('communication_call')) {
        const get = JSON.parse(byId('communication_get').content);
        const description = get.structuredContent ?? get;
        if (!description.des_id) throw Error('description token missing');
        call('communication_call',meta('mcp_call'),{
          tool_id:description.tool_id,des_id:description.des_id,arguments:{conversation_id:session.conversation_id}
        });
      } else if (session.user_id === 'alice' && !byId('communication_foreign')) {
        const get = JSON.parse(byId('communication_get').content);
        const description = get.structuredContent ?? get;
        call('communication_foreign',meta('mcp_call'),{
          tool_id:description.tool_id,des_id:description.des_id,
          arguments:{conversation_id:configuredText.split(':')[1]}
        });
      } else {
        const foreign = byId('communication_foreign');
        if (foreign && foreign.content.includes('Bob private input')) throw Error('resume lost sender scope');
        const history = byId('communication_call');
        const own = session.user_id === 'alice' ? 'Alice private input' : 'Bob private input';
        const other = session.user_id === 'alice' ? 'Bob private input' : 'Alice private input';
        if (history.is_error || !history.content.includes(own) || history.content.includes(other)) throw Error('scoped history violated');
        process.stdout.write(JSON.stringify({type:'result',result:{
          final_content:'Final for ' + session.user_id,finish_reason:'stop',usage:{input_tokens:5,output_tokens:7,total_tokens:12}
        }}) + '\n');
      }
      streamed = true;
      break;
    }
