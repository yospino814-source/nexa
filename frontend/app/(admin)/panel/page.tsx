import type { Metadata } from "next";

import { Topbar } from "@/components/nexa/topbar";
import { PanelScreen } from "@/components/panel/panel-screen";

export const metadata: Metadata = { title: "Panel principal · Nexa POS" };

export default function PanelPage() {
  return (
    <>
      <Topbar title="Panel Principal" />
      <PanelScreen />
    </>
  );
}
