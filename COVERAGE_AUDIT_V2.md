# multipeerconnectivity-rs coverage audit v2 (vs MacOSX26.2.sdk)

SDK_PUBLIC_SYMBOLS: 78
VERIFIED: 78
GAPS: 0
EXEMPT: 0
COVERAGE_PCT: 100.00%

This audit enumerates Objective-C public surface members (methods, properties, exported constants, and enum cases) from `MultipeerConnectivity.framework` headers at MacOSX26.2.sdk. Container declarations such as `@interface`, `@protocol`, and enum type names are not counted separately, but the five public delegate protocol declarations are explicitly documented in a separate verification table below because the crate exposes them as Rust callback builders and async event streams where applicable. Delegate properties are treated as covered by the crate's callback-registration APIs, and `PeerId::archived_data` / `PeerId::from_archived_data` are extra `NSSecureCoding` helpers that sit outside the header-derived symbol count.

## 🟢 VERIFIED
| Symbol | Kind | Header | Wrapped by |
| --- | --- | --- | --- |
| `-initWithDisplayName:` | method | `MCPeerID.h` | `PeerId::new` |
| `displayName` | property | `MCPeerID.h` | `PeerId::display_name` |
| `MCErrorDomain` | constant | `MCError.h` | `mc_error_domain` |
| `MCErrorUnknown` | enum case | `MCError.h` | `MCErrorCode::Unknown` |
| `MCErrorNotConnected` | enum case | `MCError.h` | `MCErrorCode::NotConnected` |
| `MCErrorInvalidParameter` | enum case | `MCError.h` | `MCErrorCode::InvalidParameter` |
| `MCErrorUnsupported` | enum case | `MCError.h` | `MCErrorCode::Unsupported` |
| `MCErrorTimedOut` | enum case | `MCError.h` | `MCErrorCode::TimedOut` |
| `MCErrorCancelled` | enum case | `MCError.h` | `MCErrorCode::Cancelled` |
| `MCErrorUnavailable` | enum case | `MCError.h` | `MCErrorCode::Unavailable` |
| `MCSessionSendDataReliable` | enum case | `MCSession.h` | `SessionSendDataMode::Reliable` |
| `MCSessionSendDataUnreliable` | enum case | `MCSession.h` | `SessionSendDataMode::Unreliable` |
| `MCSessionStateNotConnected` | enum case | `MCSession.h` | `SessionState::NotConnected` |
| `MCSessionStateConnecting` | enum case | `MCSession.h` | `SessionState::Connecting` |
| `MCSessionStateConnected` | enum case | `MCSession.h` | `SessionState::Connected` |
| `MCEncryptionOptional` | enum case | `MCSession.h` | `EncryptionPreference::Optional` |
| `MCEncryptionRequired` | enum case | `MCSession.h` | `EncryptionPreference::Required` |
| `MCEncryptionNone` | enum case | `MCSession.h` | `EncryptionPreference::None` |
| `kMCSessionMinimumNumberOfPeers` | constant | `MCSession.h` | `session_minimum_number_of_peers` |
| `kMCSessionMaximumNumberOfPeers` | constant | `MCSession.h` | `session_maximum_number_of_peers` |
| `-initWithPeer:` | method | `MCSession.h` | `Session::new` |
| `-initWithPeer:securityIdentity:encryptionPreference:` | method | `MCSession.h` | `Session::with_security_identity`, `Session::with_security_identity_items` |
| `-sendData:toPeers:withMode:error:` | method | `MCSession.h` | `Session::send` |
| `-disconnect` | method | `MCSession.h` | `Session::disconnect` |
| `-sendResourceAtURL:withName:toPeer:withCompletionHandler:` | method | `MCSession.h` | `Session::send_resource`, `Session::send_resource_with_completion` |
| `-startStreamWithName:toPeer:error:` | method | `MCSession.h` | `Session::start_stream` |
| `delegate` | property | `MCSession.h` | `Session::set_delegate`, `Session::set_callbacks`, `Session::clear_delegate` |
| `myPeerID` | property | `MCSession.h` | `Session::my_peer_id` |
| `securityIdentity` | property | `MCSession.h` | `Session::security_identity` |
| `encryptionPreference` | property | `MCSession.h` | `Session::encryption_preference` |
| `connectedPeers` | property | `MCSession.h` | `Session::connected_peers` |
| `-session:peer:didChangeState:` | protocol method | `MCSession.h` | `SessionDelegate::on_state` |
| `-session:didReceiveData:fromPeer:` | protocol method | `MCSession.h` | `SessionDelegate::on_data` |
| `-session:didReceiveStream:withName:fromPeer:` | protocol method | `MCSession.h` | `SessionDelegate::on_stream`, `InputStream` |
| `-session:didStartReceivingResourceWithName:fromPeer:withProgress:` | protocol method | `MCSession.h` | `SessionDelegate::on_resource_started`, `ResourceTransfer` |
| `-session:didFinishReceivingResourceWithName:fromPeer:atURL:withError:` | protocol method | `MCSession.h` | `SessionDelegate::on_resource_finished` |
| `-session:didReceiveCertificate:fromPeer:certificateHandler:` | protocol method | `MCSession.h` | `SessionDelegate::on_certificate` |
| `-nearbyConnectionDataForPeer:withCompletionHandler:` | method | `MCSession.h` | `Session::nearby_connection_data_for_peer` |
| `-connectPeer:withNearbyConnectionData:` | method | `MCSession.h` | `Session::connect_peer` |
| `-cancelConnectPeer:` | method | `MCSession.h` | `Session::cancel_connect_peer` |
| `-initWithPeer:discoveryInfo:serviceType:` | method | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiser::new` |
| `-startAdvertisingPeer` | method | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiser::start_advertising_peer` |
| `-stopAdvertisingPeer` | method | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiser::stop_advertising_peer` |
| `delegate` | property | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiser::set_delegate`, `NearbyServiceAdvertiser::set_callbacks`, `NearbyServiceAdvertiser::clear_delegate` |
| `myPeerID` | property | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiser::my_peer_id` |
| `discoveryInfo` | property | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiser::discovery_info` |
| `serviceType` | property | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiser::service_type` |
| `-advertiser:didReceiveInvitationFromPeer:withContext:invitationHandler:` | protocol method | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiserDelegate::on_invitation`, `InvitationResponse` |
| `-advertiser:didNotStartAdvertisingPeer:` | protocol method | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiserDelegate::on_error` |
| `-initWithPeer:serviceType:` | method | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowser::new` |
| `-startBrowsingForPeers` | method | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowser::start_browsing_for_peers` |
| `-stopBrowsingForPeers` | method | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowser::stop_browsing_for_peers` |
| `-invitePeer:toSession:withContext:timeout:` | method | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowser::invite_peer` |
| `delegate` | property | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowser::set_delegate`, `NearbyServiceBrowser::set_callbacks`, `NearbyServiceBrowser::clear_delegate` |
| `myPeerID` | property | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowser::my_peer_id` |
| `serviceType` | property | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowser::service_type` |
| `-browser:foundPeer:withDiscoveryInfo:` | protocol method | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowserDelegate::on_found` |
| `-browser:lostPeer:` | protocol method | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowserDelegate::on_lost` |
| `-browser:didNotStartBrowsingForPeers:` | protocol method | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowserDelegate::on_error` |
| `-initWithServiceType:discoveryInfo:session:` | method | `MCAdvertiserAssistant.h` | `AdvertiserAssistant::new` |
| `-start` | method | `MCAdvertiserAssistant.h` | `AdvertiserAssistant::start` |
| `-stop` | method | `MCAdvertiserAssistant.h` | `AdvertiserAssistant::stop` |
| `delegate` | property | `MCAdvertiserAssistant.h` | `AdvertiserAssistant::set_callbacks`, `AdvertiserAssistant::clear_delegate` |
| `session` | property | `MCAdvertiserAssistant.h` | `AdvertiserAssistant::session` |
| `discoveryInfo` | property | `MCAdvertiserAssistant.h` | `AdvertiserAssistant::discovery_info` |
| `serviceType` | property | `MCAdvertiserAssistant.h` | `AdvertiserAssistant::service_type` |
| `-advertiserAssistantWillPresentInvitation:` | protocol method | `MCAdvertiserAssistant.h` | `AdvertiserAssistantDelegate::on_will_present_invitation` |
| `-advertiserAssistantDidDismissInvitation:` | protocol method | `MCAdvertiserAssistant.h` | `AdvertiserAssistantDelegate::on_did_dismiss_invitation` |
| `-initWithServiceType:session:` | method | `MCBrowserViewController.h` | `BrowserViewController::new_with_service_type` |
| `-initWithBrowser:session:` | method | `MCBrowserViewController.h` | `BrowserViewController::new_with_browser` |
| `delegate` | property | `MCBrowserViewController.h` | `BrowserViewController::set_callbacks`, `BrowserViewController::clear_delegate` |
| `browser` | property | `MCBrowserViewController.h` | `BrowserViewController::browser` |
| `session` | property | `MCBrowserViewController.h` | `BrowserViewController::session` |
| `minimumNumberOfPeers` | property | `MCBrowserViewController.h` | `BrowserViewController::minimum_number_of_peers`, `BrowserViewController::set_minimum_number_of_peers` |
| `maximumNumberOfPeers` | property | `MCBrowserViewController.h` | `BrowserViewController::maximum_number_of_peers`, `BrowserViewController::set_maximum_number_of_peers` |
| `-browserViewControllerDidFinish:` | protocol method | `MCBrowserViewController.h` | `BrowserViewControllerDelegate::on_finish` |
| `-browserViewControllerWasCancelled:` | protocol method | `MCBrowserViewController.h` | `BrowserViewControllerDelegate::on_cancel` |
| `-browserViewController:shouldPresentNearbyPeer:withDiscoveryInfo:` | protocol method | `MCBrowserViewController.h` | `BrowserViewControllerDelegate::should_present_peer` |

## 🟢 VERIFIED DELEGATE PROTOCOL DECLARATIONS (not counted)
| Protocol | Header | Wrapped by |
| --- | --- | --- |
| `MCAdvertiserAssistantDelegate` | `MCAdvertiserAssistant.h` | `AdvertiserAssistantDelegate`, `AdvertiserAssistant::set_callbacks` |
| `MCBrowserViewControllerDelegate` | `MCBrowserViewController.h` | `BrowserViewControllerDelegate`, `BrowserViewController::set_callbacks` |
| `MCNearbyServiceAdvertiserDelegate` | `MCNearbyServiceAdvertiser.h` | `NearbyServiceAdvertiserDelegate`, `NearbyServiceAdvertiser::set_callbacks`, `async_api::AdvertiserEvent`, `async_api::AdvertiserEventStream` |
| `MCNearbyServiceBrowserDelegate` | `MCNearbyServiceBrowser.h` | `NearbyServiceBrowserDelegate`, `NearbyServiceBrowser::set_callbacks`, `async_api::BrowserEvent`, `async_api::BrowserEventStream` |
| `MCSessionDelegate` | `MCSession.h` | `SessionDelegate`, `Session::set_callbacks`, `async_api::SessionEvent`, `async_api::SessionEventStream` |

## 🔴 GAPS
| Symbol | Kind | Header | Notes |
| --- | --- | --- | --- |
_No gaps._

## ⏭️ EXEMPT
| Symbol | Kind | Header | Reason | SDK attribute |
| --- | --- | --- | --- | --- |
_No exempt symbols._
