# Optional green-path measurement; uses source line tables already present in
# the original CP binary. Does not rebuild or change the test stack/profile.
set pagination off
set confirm off
set breakpoint pending off
python
import gdb, os
class SelectionBoundary(gdb.Breakpoint):
    def stop(self):
        try:
            thread = gdb.selected_thread()
            if not thread or not (thread.name or '').startswith('application-pub'):
                return False
            provider = False
            route = False
            frame = gdb.newest_frame()
            for _ in range(160):
                if frame is None:
                    break
                sal = frame.find_sal()
                filename = sal.symtab.filename if sal.symtab else ''
                provider |= filename.endswith('/orchestration_runtime/provider_invoker/provider_execution.rs')
                route |= filename.endswith('/routes/application_public_api/openai.rs') or filename.endswith('/routes/application_public_api/anthropic.rs')
                frame = frame.older()
            if provider and route:
                print('SELECTION_BOUNDARY_MATCHED: runtime_event_persister.rs:132 with provider-execution and real route ancestors')
                return True
            return False
        except Exception as error:
            print('SELECTION_BOUNDARY_CHECK_UNAVAILABLE', str(error)[:250])
            return False
try:
    SelectionBoundary('crates/control-plane/src/orchestration_runtime/persistence/runtime_event_persister.rs:132', internal=True)
    print('SELECTION_BOUNDARY_ARMED')
except Exception as error:
    print('SELECTION_BOUNDARY_UNAVAILABLE', str(error)[:250])
# Reuse the already validated bounded capture. If SIGSEGV occurs before this
# breakpoint it still captures that fault; PROGRAM and MATCHED distinguish cases.
gdb.execute('source ' + os.environ['GITHUB_WORKSPACE'] + '/.github/lean-first-fault.gdb')
end
