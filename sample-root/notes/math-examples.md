# Math Examples

This document demonstrates the math rendering capabilities in Mindex.

## Inline Math

The quadratic formula $x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$ solves $ax^2 + bx + c = 0$.

Einstein's famous equation $E = mc^2$ relates energy and mass.

The area of a circle is $A = \pi r^2$ where $r$ is the radius.

## Display Math

The Pythagorean theorem:

$$a^2 + b^2 = c^2$$

A more complex integral:

$$\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}$$

Euler's identity:

$$e^{i\pi} + 1 = 0$$

## Greek Letters

Greek letters are common in math: $\alpha$, $\beta$, $\gamma$, $\delta$, $\epsilon$, $\theta$, $\lambda$, $\mu$, $\sigma$, $\omega$.

Capital Greek: $\Gamma$, $\Delta$, $\Theta$, $\Lambda$, $\Sigma$, $\Omega$.

## Summations and Products

Sum notation: $\sum_{i=1}^{n} i = \frac{n(n+1)}{2}$

Product notation: $\prod_{i=1}^{n} i = n!$

Display sum:

$$\sum_{k=0}^{\infty} \frac{x^k}{k!} = e^x$$

## Matrices

A 2x2 matrix:

$$\begin{pmatrix} a & b \\ c & d \end{pmatrix}$$

## Limits

$$\lim_{x \to \infty} \frac{1}{x} = 0$$

$$\lim_{n \to \infty} \left(1 + \frac{1}{n}\right)^n = e$$

## Fractions and Binomials

Nested fractions: $\frac{1}{1 + \frac{1}{x}}$

Binomial coefficient: $\binom{n}{k} = \frac{n!}{k!(n-k)!}$

## Supported LaTeX

This uses the `latex2mathml` library which supports most common LaTeX math commands. Some advanced features may not render correctly - in that case, the raw LaTeX will be shown as a fallback.