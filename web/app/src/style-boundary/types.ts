import type { ReactElement } from 'react';

export type StyleBoundarySceneKind = 'component' | 'page';

export interface StyleBoundaryPropertyAssertion {
  property: string;
  expected: string;
}

export interface StyleBoundaryTargetNode {
  id: string;
  selector: string;
  propertyAssertions: StyleBoundaryPropertyAssertion[];
}

export type StyleBoundaryRelationshipAssertion =
  | {
      id: string;
      type: 'no_overlap';
      subjectSelector: string;
      referenceSelector: string;
    }
  | {
      id: string;
      type: 'within_container';
      subjectSelector: string;
      containerSelector: string;
    }
  | {
      id: string;
      type: 'min_gap';
      subjectSelector: string;
      referenceSelector: string;
      minGap: number;
      axis?: 'horizontal' | 'vertical';
    }
  | {
      id: string;
      type: 'fully_visible';
      subjectSelector: string;
    };

export interface StyleBoundaryManifestScene {
  id: string;
  kind: StyleBoundarySceneKind;
  title: string;
  impactFiles: string[];
  boundaryNodes: StyleBoundaryTargetNode[];
  viewport?: {
    width: number;
    height: number;
  };
  relationshipAssertions?: StyleBoundaryRelationshipAssertion[];
}

export interface StyleBoundaryRuntimeScene extends StyleBoundaryManifestScene {
  render: () => ReactElement;
}
