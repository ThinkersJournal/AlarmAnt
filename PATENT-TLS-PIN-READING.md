# Patent reading for server TLS and a fingerprint pin in the enrolment link (slice 2 of the https-only work)

Date: 2026-11-19. Run by the AlarmAnt lane (Sonnet 5.5). **A model reading, not legal advice and not a freedom-to-operate opinion; there is no attorney.** Claims read with `tools/patent-claims.py` from Google Patents (page text, not the official USPTO record). The sweep as a whole stays incomplete; say "outside as read", never "clear". USPTO full text, Espacenet and PATENTSCOPE were not run.

**The change read:** the enrol link `alarmant://enroll?u=<https base url>&c=<code>` gains one optional parameter `f=<SHA-256 of the server certificate, hex>`. The server makes a self-signed certificate at first run and prints the link with `f`. The phone, for a link with `f`, accepts the server certificate only if its SHA-256 equals `f` (a pin held with the stored base URL); a link without `f` uses the system trust store. No client certificate, no CSR, no certificate authority, no config file, no rotation protocol.

## What was searched

| # | Query (US web search) | Result |
|---|---|---|
| T1 | mobile device enrollment link containing server certificate fingerprint, pinned self-signed certificate, one-time code | US10397778, US9674173, US9930028, US7444508, US2020/0396610, US2018/0367530 |
| T2 | certificate pinning, provisioning mobile application, server public-key hash in invitation QR code | US10356087 / US11451535 (Hypori), US2018/0302787 (Synchronoss), US11139988 |

## Claims read (independents)

| Patent | Holder; status (Google, not official) | Independents read | What they need | Verdict for the designed change |
|---|---|---|---|---|
| US2018/0302787A1 HTTPS pin provisioning | Synchronoss; **Abandoned** | 1, 8, 13, 17 | the server **generates a key pin from its certificate and stores it in a mobile-client configuration file that is digitally signed**; the phone **verifies that signature with a key from a public key store** before storing pins; (13) the phone compares an SPKI hash with a stored pin | outside as read: no configuration file, no signature, no public key store; our pin is one value in a link the operator hands over, and it is not a SPKI hash |
| US9930028B2 certificate enrolment via SCEP | Thomson/InterDigital; Expired (fee) | 1, 8, 12, 13 | a **management application** discovering a device and forwarding its discovery information to a **certificate enrolment server**, which signs the device's request | outside as read (and expired): no management application, no discovery, no certificate issued to the phone |
| US11451535B1 credential provisioning | Hypori; Active, est. 2038-03-19 | 1, 8, 15 | **generating a key pair and a CSR, sending it to a certificate authority, receiving a signed certificate**, encrypting it under a temporary password and returning it to the client for a key store | outside as read: no key pair for the phone, no CSR, no CA, no client certificate |
| US2020/0396610A1 trusted secret package | Motorola Solutions; Active, est. 2038-05-24 | 1, 13 | a phone scanning a **machine-readable optical label** carrying authentication data and security information, **generating a certificate request signed with them**, sent to a **registration authority** | outside as read: our link may be pasted or later shown as a QR, but the phone makes **no certificate request** and nothing is signed with the link's contents; it is a bearer code plus a fingerprint compared to the server's certificate |

Not read (titles from snippets only): US10397778, US9674173, US7444508 (user compares part of a fingerprint by hand), US2018/0367530 (certificate set from DHCP), US11139988.

## Constraints this reading adds

T1. The phone never generates a key pair, a certificate request or a signature for enrolment, and the server issues no certificate to the phone.
T2. No signed configuration file and no public key store for pins; the pin is one value in the enrol link, nothing else is provisioned.
T3. No pin rotation or "future pin" scheme; a new server certificate means a new enrolment link.
T4. No management or discovery application; the operator copies the link.
T5. Re-read before: client certificates, a QR/optical-label flow that carries more than the link, any signed pin update.

Whether this needs the PM or CireSnave beyond pm-ants's go of 2026-11-19 is theirs to say; this file is the written check it asked for.
