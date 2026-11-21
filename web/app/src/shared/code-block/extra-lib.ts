export interface BlockSourceExtraLib {
  source: string;
  filePath: string;
  content: string;
}

export interface BlockSourceDeclarationLoader {
  loadForSource(source: string): Promise<readonly BlockSourceExtraLib[]>;
  loadAll(): Promise<readonly BlockSourceExtraLib[]>;
}
