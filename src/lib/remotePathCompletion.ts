export type RemotePathCompletionContext = {
  directoryInput: string;
  typedPrefix: string;
  valuePrefix: string;
};

export function getRemotePathCompletionContext(input: string): RemotePathCompletionContext {
  const lastSlash = input.lastIndexOf('/');
  if (lastSlash < 0) {
    return {
      directoryInput: '.',
      typedPrefix: input,
      valuePrefix: '',
    };
  }

  const valuePrefix = input.slice(0, lastSlash + 1);
  const directoryInput = input.slice(0, lastSlash) || '/';
  return {
    directoryInput,
    typedPrefix: input.slice(lastSlash + 1),
    valuePrefix,
  };
}

export function buildRemotePathCompletion(valuePrefix: string, directoryName: string): string {
  return `${valuePrefix}${directoryName}/`;
}
