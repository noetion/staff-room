import { useEffect, useState } from "react";

type CachedDisplacementMap = {
  width: number;
  height: number;
  dataUrl: string;
};

const initialMapWidth = 512;
const initialMapHeight = 128;
const bezelStart = 0.86;
const bezelDepth = 0.14;
const channelNeutral = 128;
const channelRange = 127;
const squirclePower = 4;
const minimumLogicalCores = 4;

function createDisplacementMap(width: number, height: number): string {
  if (typeof document === "undefined") return "";

  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, width);
  canvas.height = Math.max(1, height);
  let context: CanvasRenderingContext2D | null;
  try {
    context = canvas.getContext("2d");
  } catch {
    return "";
  }
  if (!context) return "";

  const image = context.createImageData(canvas.width, canvas.height);
  const widthDenominator = Math.max(1, canvas.width - 1);
  const heightDenominator = Math.max(1, canvas.height - 1);

  for (let row = 0; row < canvas.height; row += 1) {
    const y = (row / heightDenominator) * 2 - 1;
    for (let column = 0; column < canvas.width; column += 1) {
      const x = (column / widthDenominator) * 2 - 1;
      const squircleRadius = (
        Math.abs(x) ** squirclePower
        + Math.abs(y) ** squirclePower
      ) ** (1 / squirclePower);
      const edge = Math.min(1, Math.max(0, (squircleRadius - bezelStart) / bezelDepth));
      const ramp = edge * edge * (3 - 2 * edge);
      const index = (row * canvas.width + column) * 4;

      image.data[index] = Math.round(channelNeutral + x * ramp * channelRange);
      image.data[index + 1] = Math.round(channelNeutral + y * ramp * channelRange);
      image.data[index + 2] = channelNeutral;
      image.data[index + 3] = 255;
    }
  }

  context.putImageData(image, 0, 0);
  try {
    return canvas.toDataURL("image/png");
  } catch {
    return "";
  }
}

let cachedDisplacementMap: CachedDisplacementMap = {
  width: initialMapWidth,
  height: initialMapHeight,
  dataUrl: createDisplacementMap(initialMapWidth, initialMapHeight),
};

function displacementMapFor(width: number, height: number): string {
  const nextWidth = Math.max(1, Math.ceil(width));
  const nextHeight = Math.max(1, Math.ceil(height));
  if (
    cachedDisplacementMap.width !== nextWidth
    || cachedDisplacementMap.height !== nextHeight
  ) {
    cachedDisplacementMap = {
      width: nextWidth,
      height: nextHeight,
      dataUrl: createDisplacementMap(nextWidth, nextHeight),
    };
  }
  return cachedDisplacementMap.dataUrl;
}

const refractionSupportedAtBoot = (
  typeof CSS !== "undefined"
  && CSS.supports("backdrop-filter", "url(#x)")
  && (
    typeof navigator === "undefined"
    || (navigator.hardwareConcurrency || minimumLogicalCores) >= minimumLogicalCores
  )
);

function glassSurface(element: HTMLElement | null): HTMLElement | null {
  if (!element) return null;
  if (element.classList.contains("glass")) return element;
  return element.querySelector<HTMLElement>(".glass");
}

function refractionSurfaces(root: HTMLElement, composerVisible: boolean): HTMLElement[] {
  const dragRegion = root.querySelector<HTMLElement>("[data-tauri-drag-region]");
  const titleBar = glassSurface(
    dragRegion?.closest<HTMLElement>(".glass")
    ?? root.querySelector<HTMLElement>(":scope > header"),
  );

  if (!composerVisible) return titleBar ? [titleBar] : [];

  const composerForm = Array.from(
    root.querySelectorAll<HTMLFormElement>("main form"),
  ).find((form) => form.querySelector("textarea")) ?? null;
  const composer = glassSurface(composerForm);
  return [titleBar, composer].filter((surface): surface is HTMLElement => Boolean(surface));
}

export type RefractionFilterProps = {
  composerVisible: boolean;
  surfaceKey: string;
};

export function RefractionFilter({ composerVisible, surfaceKey }: RefractionFilterProps) {
  const [mapDataUrl, setMapDataUrl] = useState(cachedDisplacementMap.dataUrl);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      document.documentElement.dataset.refraction = "off";
      return;
    }

    const reducedTransparency = window.matchMedia("(prefers-reduced-transparency: reduce)");
    const forcedColors = window.matchMedia("(forced-colors: active)");
    const updateFlag = () => {
      document.documentElement.dataset.refraction = (
        refractionSupportedAtBoot
        && !reducedTransparency.matches
        && !forcedColors.matches
      ) ? "on" : "off";
    };

    updateFlag();
    reducedTransparency.addEventListener("change", updateFlag);
    forcedColors.addEventListener("change", updateFlag);
    return () => {
      reducedTransparency.removeEventListener("change", updateFlag);
      forcedColors.removeEventListener("change", updateFlag);
    };
  }, []);

  useEffect(() => {
    const root = document.querySelector<HTMLElement>(".app-shell");
    if (!root) return;

    const surfaces = refractionSurfaces(root, composerVisible);
    surfaces.forEach((surface) => surface.classList.add("glass--refract"));

    const updateMap = () => {
      const bounds = surfaces.map((surface) => surface.getBoundingClientRect());
      if (!bounds.length) return;
      setMapDataUrl(displacementMapFor(
        Math.max(...bounds.map((bound) => bound.width)),
        Math.max(...bounds.map((bound) => bound.height)),
      ));
    };

    updateMap();
    const resizeDelay = Number.parseFloat(
      getComputedStyle(document.documentElement)
        .getPropertyValue("--refraction-resize-delay"),
    );
    if (typeof ResizeObserver === "undefined") {
      return () => {
        surfaces.forEach((surface) => surface.classList.remove("glass--refract"));
      };
    }
    let resizeTimer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      if (resizeTimer !== undefined) clearTimeout(resizeTimer);
      resizeTimer = setTimeout(updateMap, resizeDelay);
    });
    surfaces.forEach((surface) => observer.observe(surface));

    return () => {
      observer.disconnect();
      if (resizeTimer !== undefined) clearTimeout(resizeTimer);
      surfaces.forEach((surface) => surface.classList.remove("glass--refract"));
    };
  }, [composerVisible, surfaceKey]);

  return (
    <svg aria-hidden="true" className="refraction-defs" width="0" height="0">
      <filter id="staff-room-refract" colorInterpolationFilters="sRGB">
        <feImage
          href={mapDataUrl}
          result="map"
          x="0"
          y="0"
          width="100%"
          height="100%"
          preserveAspectRatio="none"
        />
        <feDisplacementMap
          in="SourceGraphic"
          in2="map"
          scale={12}
          xChannelSelector="R"
          yChannelSelector="G"
        />
      </filter>
    </svg>
  );
}
