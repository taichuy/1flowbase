const USER_SCROLL_QUIET_MS = 150;
const SCROLL_OFFSET_TOLERANCE_PX = 1.5;

export interface FrontstageScrollIntentArbiter {
  applyProgrammaticScroll(targetTop: number): boolean;
  applyLayoutCompensation(
    capturedScrollTop: number,
    displacement: number
  ): boolean;
}

export interface FrontstageScrollIntentArbiterLease {
  readonly arbiter: FrontstageScrollIntentArbiter;
  release(): void;
}

type ScrollOwner = HTMLElement | Window;

interface SharedArbiterEntry {
  arbiter: DomScrollIntentArbiter;
  leases: number;
}

const sharedArbiters = new WeakMap<object, SharedArbiterEntry>();

export function acquireFrontstageScrollIntentArbiter(
  scrollOwner: ScrollOwner,
  ownerWindow: Window
): FrontstageScrollIntentArbiterLease {
  let entry = sharedArbiters.get(scrollOwner);
  if (!entry) {
    entry = {
      arbiter: new DomScrollIntentArbiter(scrollOwner, ownerWindow),
      leases: 0
    };
    sharedArbiters.set(scrollOwner, entry);
  }
  entry.leases += 1;
  let released = false;
  return {
    arbiter: entry.arbiter,
    release() {
      if (released) return;
      released = true;
      entry.leases -= 1;
      if (entry.leases > 0) return;
      entry.arbiter.dispose();
      sharedArbiters.delete(scrollOwner);
    }
  };
}

export function applyFrontstageLayoutCompensation(
  scrollOwner: HTMLElement,
  capturedScrollTop: number,
  displacement: number
): boolean {
  const shared = sharedArbiters.get(scrollOwner);
  if (shared) {
    return shared.arbiter.applyLayoutCompensation(
      capturedScrollTop,
      displacement
    );
  }
  if (
    Math.abs(scrollOwner.scrollTop - capturedScrollTop) >
    SCROLL_OFFSET_TOLERANCE_PX
  ) {
    return false;
  }
  scrollOwner.scrollTo({
    top: scrollOwner.scrollTop + displacement,
    behavior: 'auto'
  });
  return true;
}

class DomScrollIntentArbiter implements FrontstageScrollIntentArbiter {
  private intendedScrollTop: number | null = null;
  private observedScrollTop: number;
  private userScrollActive = false;
  private quietTimer: number | null = null;

  constructor(
    private readonly scrollOwner: ScrollOwner,
    private readonly ownerWindow: Window
  ) {
    this.observedScrollTop = readScrollTop(scrollOwner, ownerWindow);
    scrollOwner.addEventListener('scroll', this.handleScroll, {
      passive: true
    });
  }

  applyProgrammaticScroll(targetTop: number): boolean {
    if (this.userScrollActive) return false;
    const nextTop = clampScrollTop(
      targetTop,
      this.scrollOwner,
      this.ownerWindow
    );
    if (
      Math.abs(readScrollTop(this.scrollOwner, this.ownerWindow) - nextTop) <=
      SCROLL_OFFSET_TOLERANCE_PX
    ) {
      return true;
    }
    this.intendedScrollTop = nextTop;
    this.scrollOwner.scrollTo({ top: nextTop, behavior: 'auto' });
    return true;
  }

  applyLayoutCompensation(
    capturedScrollTop: number,
    displacement: number
  ): boolean {
    const currentTop = readScrollTop(this.scrollOwner, this.ownerWindow);
    if (Math.abs(currentTop - capturedScrollTop) > SCROLL_OFFSET_TOLERANCE_PX) {
      return false;
    }
    return this.applyProgrammaticScroll(currentTop + displacement);
  }

  dispose(): void {
    this.scrollOwner.removeEventListener('scroll', this.handleScroll);
    if (this.quietTimer !== null) {
      this.ownerWindow.clearTimeout(this.quietTimer);
      this.quietTimer = null;
    }
  }

  private readonly handleScroll = () => {
    const currentTop = readScrollTop(this.scrollOwner, this.ownerWindow);
    if (
      this.intendedScrollTop !== null &&
      Math.abs(currentTop - this.intendedScrollTop) <=
        SCROLL_OFFSET_TOLERANCE_PX
    ) {
      this.observedScrollTop = currentTop;
      this.intendedScrollTop = null;
      return;
    }
    this.intendedScrollTop = null;
    if (
      Math.abs(currentTop - this.observedScrollTop) <=
      SCROLL_OFFSET_TOLERANCE_PX
    ) {
      return;
    }
    this.observedScrollTop = currentTop;
    this.userScrollActive = true;
    if (this.quietTimer !== null) {
      this.ownerWindow.clearTimeout(this.quietTimer);
    }
    this.quietTimer = this.ownerWindow.setTimeout(() => {
      this.userScrollActive = false;
      this.quietTimer = null;
    }, USER_SCROLL_QUIET_MS);
  };
}

function readScrollTop(scrollOwner: ScrollOwner, ownerWindow: Window): number {
  return scrollOwner === ownerWindow
    ? ownerWindow.scrollY
    : (scrollOwner as HTMLElement).scrollTop;
}

function clampScrollTop(
  targetTop: number,
  scrollOwner: ScrollOwner,
  ownerWindow: Window
): number {
  const maxScrollTop =
    scrollOwner === ownerWindow
      ? Math.max(
          ownerWindow.document.documentElement.scrollHeight,
          ownerWindow.document.body.scrollHeight
        ) - ownerWindow.innerHeight
      : (scrollOwner as HTMLElement).scrollHeight -
        (scrollOwner as HTMLElement).clientHeight;
  return Math.min(Math.max(0, targetTop), Math.max(0, maxScrollTop));
}
