type RefreshQuery = () => Promise<unknown>;

type RefreshCurrentFrontstagePageInput = {
  refreshPageContent: RefreshQuery;
  refreshBlockRoots: RefreshQuery;
  refreshBlockRuntimeAssembly?: RefreshQuery;
};

export async function refreshCurrentFrontstagePage({
  refreshPageContent,
  refreshBlockRoots,
  refreshBlockRuntimeAssembly
}: RefreshCurrentFrontstagePageInput): Promise<void> {
  const outcomes = await Promise.allSettled([
    refreshPageContent(),
    refreshBlockRoots(),
    ...(refreshBlockRuntimeAssembly ? [refreshBlockRuntimeAssembly()] : [])
  ]);
  const failedOutcome = outcomes.find(
    (outcome): outcome is PromiseRejectedResult =>
      outcome.status === 'rejected'
  );
  if (failedOutcome) throw failedOutcome.reason;
}
