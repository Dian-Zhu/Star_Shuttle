function fallbackRandomUuid(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (char) => {
    const rand = Math.floor(Math.random() * 16);
    const value = char === 'x' ? rand : (rand & 0x3) | 0x8;
    return value.toString(16);
  });
}

if (!globalThis.crypto) {
  Object.defineProperty(globalThis, 'crypto', {
    value: {
      randomUUID: fallbackRandomUuid,
    },
    configurable: true,
  });
}

if (typeof globalThis.crypto.randomUUID !== 'function') {
  Object.defineProperty(globalThis.crypto, 'randomUUID', {
    value: fallbackRandomUuid,
    configurable: true,
  });
}

if (typeof globalThis.CustomEvent === 'undefined') {
  class TestCustomEvent<T = unknown> extends Event implements CustomEvent<T> {
    detail: T;

    constructor(type: string, eventInitDict?: CustomEventInit<T>) {
      super(type, eventInitDict);
      this.detail = eventInitDict?.detail as T;
    }

    initCustomEvent(
      _type: string,
      _bubbles?: boolean,
      _cancelable?: boolean,
      detail?: T
    ): void {
      this.detail = detail as T;
    }
  }

  Object.defineProperty(globalThis, 'CustomEvent', {
    value: TestCustomEvent,
    configurable: true,
  });
}
