/**
 * shadcn/ui ComponentKit.
 *
 * Implements the `ComponentKit` contract from `@/contracts` using shadcn/ui
 * components (see `@/components/ui/*`), mapping each A2UI component type to its
 * shadcn rendering. The render core (track V) looks a component up by name and
 * feeds it the normalized props defined by the A2UI protocol; swapping UI
 * libraries only means swapping the kit.
 *
 * @example
 * ```tsx
 * import { shadcnKit } from "@/kits/shadcn";
 *
 * const { Button } = shadcnKit;
 * <Button label="Save" variant="primary" disabled={false} onAction={save} />;
 * ```
 */
import type { ComponentKit } from "@/contracts";

import { Button } from "./Button";
import { Card } from "./Card";
import { Column } from "./Column";
import { Placeholder } from "./Placeholder";
import { Row } from "./Row";
import { Text } from "./Text";
import { TextField } from "./TextField";

/** The shadcn/ui implementation of the A2UI `ComponentKit` contract. */
export const shadcnKit: ComponentKit = {
  Text,
  Button,
  TextField,
  Card,
  Column,
  Row,
  Placeholder,
};

export { Text, Button, TextField, Card, Column, Row, Placeholder };
