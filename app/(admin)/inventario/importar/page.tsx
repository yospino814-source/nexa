import { ImportScreen } from "@/components/inventario/import-screen";

export const metadata = {
  title: "Importar Productos | Nexa POS",
  description: "Importación masiva de productos desde archivo CSV",
};

export default function ImportPage() {
  return <ImportScreen />;
}
