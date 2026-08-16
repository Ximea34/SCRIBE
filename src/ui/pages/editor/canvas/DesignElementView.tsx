import { memo } from "react";

import type { DesignElement } from "../../../../types/bindings";
import { cx } from "../../../cx";

import { pointsToPixels } from "./view";

import styles from "./Canvas.module.css";

interface DesignElementViewProps {
  element: DesignElement;
  scale: number;
  selected: boolean;
}

export const DesignElementView = memo(function DesignElementView({
  element,
  scale,
  selected,
}: DesignElementViewProps) {
  const common = {
    "data-drag": "element",
    "data-id": element.id,
    className: cx(styles.item, selected && styles.selected),
  } as const;
  const origin = {
    left: `${String(element.xMm * scale)}px`,
    top: `${String(element.yMm * scale)}px`,
  };

  switch (element.kind) {
    case "line": {
      const horizontal = element.orientation === "horizontal";
      return (
        <div
          {...common}
          style={{
            ...origin,
            width: `${String((horizontal ? element.lengthMm : element.thicknessMm) * scale)}px`,
            height: `${String((horizontal ? element.thicknessMm : element.lengthMm) * scale)}px`,
            background: "currentColor",
          }}
        />
      );
    }
    case "frame":
      return (
        <div
          {...common}
          style={{
            ...origin,
            width: `${String(element.widthMm * scale)}px`,
            height: `${String(element.heightMm * scale)}px`,
            border: `${String(Math.max(1, element.thicknessMm * scale))}px solid currentColor`,
          }}
        />
      );
    case "text":
      return (
        <div
          {...common}
          style={{ ...origin, fontSize: `${String(pointsToPixels(element.fontSizePt, scale))}px` }}
        >
          {element.content}
        </div>
      );
    case "image":
      return (
        <img
          {...common}
          alt=""
          draggable={false}
          src={element.data ? `data:${element.mime};base64,${element.data}` : undefined}
          style={{
            ...origin,
            width: `${String(element.widthMm * scale)}px`,
            height: `${String(element.heightMm * scale)}px`,
          }}
        />
      );
  }
});
