import { RouterProvider } from "react-router-dom";
import { router } from "./router";

export function App() {
  return (
    <div className="grain">
      <RouterProvider router={router} />
    </div>
  );
}
