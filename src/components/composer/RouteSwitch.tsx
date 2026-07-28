import { Segmented } from "../primitives";

export type ComposerRoute = "ask" | "quick-edit" | "ship";

type RouteSwitchProps = {
  value: ComposerRoute;
  onChange: (value: ComposerRoute) => void;
  quickEditDisabled: boolean;
  shipDisabled: boolean;
};

export function RouteSwitch({ value, onChange, quickEditDisabled, shipDisabled }: RouteSwitchProps) {
  return (
    <Segmented
      aria-label="Message route"
      value={value}
      onChange={onChange}
      options={[
        { value: "ask", label: "Ask" },
        { value: "quick-edit", label: "Quick edit", disabled: quickEditDisabled },
        { value: "ship", label: "Ship", disabled: shipDisabled },
      ]}
    />
  );
}
