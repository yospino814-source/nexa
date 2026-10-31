import { notFound } from "next/navigation";

import { StitchScreen } from "@/components/nexa/stitch-screen";
import { pendingScreens, screenByRoute } from "@/lib/screens";

// Rutas de las pantallas aun no migradas a React: muestran el diseno de Stitch.
// Las pantallas ya migradas tienen su propia pagina, que tiene prioridad sobre
// este catch-all.

export function generateStaticParams() {
  return pendingScreens.map((screen) => ({
    slug: screen.route.split("/").filter(Boolean),
  }));
}

export default async function ReferenceScreenPage({
  params,
}: {
  params: Promise<{ slug: string[] }>;
}) {
  const { slug } = await params;
  const screen = screenByRoute(`/${slug.join("/")}`);
  if (!screen || screen.status !== "diseno") notFound();

  return <StitchScreen screen={screen} />;
}
