const { isAwaitedAsyncInteraction, isOrderedCleanupImport, isRendererRegistryFactory, isUnitHarness, isEffectEventDependencyException } = require('./ast.js');

// Keep the original rule identity, metadata, options, listeners and diagnostics.
// Only a proven semantic counterexample can decline a particular report.
function refine(rule, exception) {
  return {
    ...rule,
    create(context) {
      const source = context.sourceCode;
      const filename = context.filename;
      const refinedContext = Object.create(context, {
        report: {
          value(...args) {
            const report = args[0];
            let accepted = false;
            try {
              accepted = report && typeof report === 'object' && exception(report, source, filename) === true;
            } catch {
              // Unknown AST/upstream report shape retains its original diagnostic.
            }
            if (!accepted) context.report(...args);
          }
        }
      });
      return rule.create(refinedContext);
    }
  };
}

function refineTestingLibraryPlugin(plugin) {
  return {
    ...plugin,
    rules: {
      ...plugin.rules,
      'no-unnecessary-act': refine(plugin.rules['no-unnecessary-act'], (report, source) =>
        report.messageId === 'noUnnecessaryActTestingLibraryUtil' && isAwaitedAsyncInteraction(report, source)),
      'no-manual-cleanup': refine(plugin.rules['no-manual-cleanup'], (report, source) =>
        report.messageId === 'noManualCleanup' && isOrderedCleanupImport(report, source)),
      'render-result-naming-convention': refine(plugin.rules['render-result-naming-convention'], (report, source, filename) =>
        isRendererRegistryFactory(report, source, filename))
    }
  };
}

function refineReactRefreshPlugin(plugin) {
  return {
    ...plugin,
    rules: {
      ...plugin.rules,
      'only-export-components': refine(plugin.rules['only-export-components'], (_report, source, filename) => isUnitHarness(source, filename))
    }
  };
}

function refineReactHooksPlugin(plugin) {
  return {
    ...plugin,
    rules: {
      ...plugin.rules,
      'exhaustive-deps': refine(plugin.rules['exhaustive-deps'], (report, source) => isEffectEventDependencyException(report, source))
    }
  };
}

module.exports = { refineTestingLibraryPlugin, refineReactRefreshPlugin, refineReactHooksPlugin };
